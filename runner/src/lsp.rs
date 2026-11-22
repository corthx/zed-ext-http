//! Language server started by the Zed extension (which is how the extension installs
//! `http-runner`). Code lenses above each request show the environments and switch the one used by
//! the ▶ tasks in a click; code actions offer the same choice. It also pre-downloads httpyac.

use crate::env::Environments;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const PICK: &str = "http-runner.pickEnvironment";
const SELECT: &str = "http-runner.selectEnvironment";
const CLEAR: &str = "http-runner.clearEnvironment";
const CLEAR_TITLE: &str = "No environment";
const INFO: u8 = 3;
const INLINE_ENVS: usize = 5;
const METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT",
];

pub fn serve() -> Result<()> {
    std::thread::spawn(|| {
        if let Err(err) = crate::httpyac::executable() {
            eprintln!("http-runner: {err:#}");
        }
    });
    let mut server = Server::default();
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    while let Some(message) = read_message(&mut input)? {
        for reply in server.handle(&message) {
            write_message(&mut output, &reply)?;
        }
        if message["method"] == "exit" {
            break;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Server {
    /// Open documents by URI.
    documents: HashMap<String, String>,
    /// Environment pickers awaiting the user's choice, by request id.
    prompts: HashMap<u64, PathBuf>,
    next_id: u64,
}

/// A successful reply plus the messages to send after it.
type Outcome = (Value, Vec<Value>);

impl Server {
    fn handle(&mut self, message: &Value) -> Vec<Value> {
        let params = &message["params"];
        let (id, method) = match (message.get("id"), message["method"].as_str()) {
            (Some(id), Some(method)) => (id, method),
            (None, Some(method)) => {
                self.notification(method, params);
                return Vec::new();
            }
            (Some(id), None) => return self.response(id, &message["result"]),
            (None, None) => return Vec::new(),
        };
        let outcome = match method {
            "initialize" => Ok((initialize_result(), Vec::new())),
            "shutdown" => Ok((Value::Null, Vec::new())),
            "textDocument/codeLens" => self.code_lenses(params).map(|v| (v, Vec::new())),
            "textDocument/codeAction" => code_actions(params).map(|v| (v, Vec::new())),
            "workspace/executeCommand" => self.execute_command(params),
            _ => {
                return vec![json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32601, "message": format!("method not supported: {method}") }
                })];
            }
        };
        match outcome {
            Ok((result, after)) => {
                let mut out = vec![json!({ "jsonrpc": "2.0", "id": id, "result": result })];
                out.extend(after);
                out
            }
            Err(err) => vec![json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32603, "message": format!("{err:#}") }
            })],
        }
    }

    fn notification(&mut self, method: &str, params: &Value) {
        let document = &params["textDocument"];
        let Some(uri) = document["uri"].as_str() else {
            return;
        };
        let text = match method {
            "textDocument/didOpen" => &document["text"],
            "textDocument/didChange" => &params["contentChanges"][0]["text"],
            "textDocument/didClose" => {
                self.documents.remove(uri);
                return;
            }
            _ => return,
        };
        if let Some(text) = text.as_str() {
            self.documents.insert(uri.to_string(), text.to_string());
        }
    }

    /// Handles the client's answer to one of our requests.
    fn response(&mut self, id: &Value, result: &Value) -> Vec<Value> {
        let Some(file) = id.as_u64().and_then(|id| self.prompts.remove(&id)) else {
            return Vec::new();
        };
        let Some(choice) = result["title"].as_str() else {
            return Vec::new();
        };
        let saved = Environments::discover(&file).and_then(|envs| {
            let envs = envs.context("no http-client.env.json found")?;
            if envs.names().iter().any(|name| name == choice) {
                envs.save_selection(Some(choice))
            } else if choice == CLEAR_TITLE {
                envs.save_selection(None)
            } else {
                bail!("unknown environment {choice}")
            }
        });
        match saved {
            Ok(()) => vec![self.request("workspace/codeLens/refresh", Value::Null)],
            Err(err) => vec![show_message(&format!("http-runner: {err:#}"))],
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        json!({ "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params })
    }

    fn code_lenses(&self, params: &Value) -> Result<Value> {
        let file = document_path(params)?;
        let Some(envs) = Environments::discover(&file)?.filter(|e| !e.names().is_empty()) else {
            return Ok(json!([]));
        };
        let commands = lens_commands(&envs, &file.to_string_lossy());
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = match self.documents.get(uri) {
            Some(text) => text.clone(),
            None => std::fs::read_to_string(&file).unwrap_or_default(),
        };
        let lenses = request_lines(&text)
            .into_iter()
            .flat_map(|line| {
                let position = json!({ "line": line, "character": 0 });
                commands.iter().map(move |command| {
                    json!({ "range": { "start": position, "end": position }, "command": command })
                })
            })
            .collect();
        Ok(Value::Array(lenses))
    }

    fn execute_command(&mut self, params: &Value) -> Result<Outcome> {
        let args = params["arguments"].as_array().cloned().unwrap_or_default();
        let file = args
            .first()
            .and_then(Value::as_str)
            .context("missing file argument")?;
        let Some(envs) = Environments::discover(Path::new(file))? else {
            bail!("no http-client.env.json found above {file}");
        };
        let note = match params["command"].as_str() {
            Some(PICK) => {
                let current = active_env(&envs);
                let mut actions: Vec<Value> = envs
                    .names()
                    .into_iter()
                    .map(|name| json!({ "title": name }))
                    .collect();
                if envs.saved_selection().is_some() {
                    actions.push(json!({ "title": CLEAR_TITLE }));
                }
                let message = match current {
                    Some(name) => format!("HTTP environment (current: {name})"),
                    None => "HTTP environment".to_string(),
                };
                let prompt = self.request(
                    "window/showMessageRequest",
                    json!({ "type": INFO, "message": message, "actions": actions }),
                );
                self.prompts.insert(self.next_id, PathBuf::from(file));
                return Ok((Value::Null, vec![prompt]));
            }
            Some(SELECT) => {
                let name = args
                    .get(1)
                    .and_then(Value::as_str)
                    .context("missing environment name")?;
                envs.check_known(name)?;
                envs.save_selection(Some(name))?;
                if args.get(2).is_some_and(|options| options["silent"] == true) {
                    let refresh = self.request("workspace/codeLens/refresh", Value::Null);
                    return Ok((Value::Null, vec![refresh]));
                }
                format!("HTTP environment: {name}")
            }
            Some(CLEAR) => {
                envs.save_selection(None)?;
                "HTTP environment cleared".into()
            }
            other => bail!("unknown command {other:?}"),
        };
        let refresh = self.request("workspace/codeLens/refresh", Value::Null);
        Ok((Value::Null, vec![show_message(&note), refresh]))
    }
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "codeLensProvider": { "resolveProvider": false },
            "codeActionProvider": true,
            "executeCommandProvider": { "commands": [PICK, SELECT, CLEAR] }
        },
        "serverInfo": { "name": "http-runner", "version": env!("CARGO_PKG_VERSION") }
    })
}

fn lens_commands(envs: &Environments, file: &str) -> Vec<Value> {
    let names = envs.names();
    let current = active_env(envs);
    if names.len() > INLINE_ENVS {
        let title = format!("env: {} ▾", current.as_deref().unwrap_or("choose…"));
        return vec![json!({ "title": title, "command": PICK, "arguments": [file] })];
    }
    names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mark = if current.as_ref() == Some(name) {
                "●"
            } else {
                "○"
            };
            let prefix = if i == 0 { "env: " } else { "" };
            json!({
                "title": format!("{prefix}{mark} {name}"),
                "command": SELECT,
                "arguments": [file, name, { "silent": true }]
            })
        })
        .collect()
}

/// The environment a ▶ run would use without prompting (same rules as the CLI).
fn active_env(envs: &Environments) -> Option<String> {
    envs.saved_selection()
        .or_else(|| match envs.names().as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        })
}

/// Zero-based lines holding a request line (`METHOD url`, or a bare URL starting a request).
fn request_lines(text: &str) -> Vec<u32> {
    let mut lines = Vec::new();
    let mut at_request_start = true;
    for (row, line) in text.lines().enumerate() {
        let line = line.trim_start();
        if line.starts_with("###") {
            at_request_start = true;
            continue;
        }
        let has_method = line
            .split_once(' ')
            .is_some_and(|(method, rest)| METHODS.contains(&method) && !rest.trim().is_empty());
        let bare_url = line.starts_with("http://") || line.starts_with("https://");
        if has_method || (at_request_start && bare_url) {
            lines.push(row as u32);
            at_request_start = false;
        } else if !(line.is_empty()
            || line.starts_with('#')
            || line.starts_with("//")
            || line.starts_with('@'))
        {
            at_request_start = false;
        }
    }
    lines
}

fn document_path(params: &Value) -> Result<PathBuf> {
    let uri = params["textDocument"]["uri"]
        .as_str()
        .context("missing textDocument.uri")?;
    let path = url::Url::parse(uri)?
        .to_file_path()
        .map_err(|()| anyhow::anyhow!("not a file URI: {uri}"))?;
    Ok(path.canonicalize().unwrap_or(path))
}

fn code_actions(params: &Value) -> Result<Value> {
    let file = document_path(params)?;
    let Some(envs) = Environments::discover(&file)? else {
        return Ok(json!([]));
    };
    let current = envs.saved_selection();
    let file_arg = file.to_string_lossy();
    let mut actions: Vec<Value> = envs
        .names()
        .into_iter()
        .map(|name| {
            let is_current = current.as_deref() == Some(name.as_str());
            let title = if is_current {
                format!("✓ HTTP environment: {name} (current)")
            } else {
                format!("HTTP: use environment \"{name}\"")
            };
            json!({
                "title": title,
                "command": { "title": title, "command": SELECT, "arguments": [file_arg, name] }
            })
        })
        .collect();
    if current.is_some() {
        let title = "HTTP: clear environment selection";
        actions.push(json!({
            "title": title,
            "command": { "title": title, "command": CLEAR, "arguments": [file_arg] }
        }));
    }
    Ok(Value::Array(actions))
}

fn show_message(text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "window/showMessage",
        "params": { "type": INFO, "message": text }
    })
}

fn read_message(input: &mut impl BufRead) -> Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = Some(value.trim().parse::<usize>()?);
        }
    }
    let Some(length) = length else {
        bail!("LSP message without Content-Length");
    };
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    Ok(Some(
        serde_json::from_slice(&body).context("invalid LSP message")?,
    ))
}

fn write_message(output: &mut impl Write, message: &Value) -> Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn frame(value: Value) -> Vec<u8> {
        let mut out = Vec::new();
        write_message(&mut out, &value).unwrap();
        out
    }

    fn request(id: u64, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    #[test]
    fn round_trips_framed_messages() {
        let mut server = Server::default();
        let mut input = frame(request(1, "initialize", json!({})));
        input.extend(frame(json!({"jsonrpc": "2.0", "method": "initialized"})));
        let mut reader = io::Cursor::new(input);

        let replies = server.handle(&read_message(&mut reader).unwrap().unwrap());
        assert_eq!(replies[0]["id"], 1);
        assert_eq!(
            replies[0]["result"]["capabilities"]["codeActionProvider"],
            true
        );

        let notification = read_message(&mut reader).unwrap().unwrap();
        assert!(
            server.handle(&notification).is_empty(),
            "notifications get no reply"
        );
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn rejects_unknown_requests() {
        let replies = Server::default().handle(&request(7, "textDocument/hover", json!({})));
        assert_eq!(replies[0]["error"]["code"], -32601);
    }

    #[test]
    fn finds_request_lines() {
        let text = "@host = x\n\
                    # comment\n\
                    GET {{host}}/a\n\
                    Accept: */*\n\
                    \n\
                    ### second\n\
                    # @name b\n\
                    https://example.com/b\n\
                    \n\
                    {\"url\": \"https://not-a-request\"}\n\
                    ###\n\
                    POST https://example.com/c HTTP/1.1\n";
        assert_eq!(request_lines(text), [2, 7, 11]);
    }

    // Single test touching the selection store, so tests cannot race on HTTP_RUNNER_CONFIG_DIR.
    #[test]
    fn selects_environment_from_code_actions_and_lenses() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HTTP_RUNNER_CONFIG_DIR", tmp.path().join("config")) };
        let root = tmp.path().canonicalize().unwrap();
        fs::write(
            root.join("http-client.env.json"),
            r#"{"dev": {}, "prod": {}}"#,
        )
        .unwrap();
        let file = root.join("api.http");
        fs::write(&file, "GET http://x").unwrap();
        let uri = url::Url::from_file_path(&file).unwrap().to_string();
        let params = json!({ "textDocument": { "uri": uri }, "range": {}, "context": { "diagnostics": [] } });
        let mut server = Server::default();
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "text": "GET http://x\n\n###\nGET http://y" } }
        }));

        let titles = |server: &mut Server| -> Vec<Value> {
            let lenses = server.handle(&request(1, "textDocument/codeLens", params.clone()));
            let lenses = lenses[0]["result"].as_array().unwrap().clone();
            assert_eq!(lenses[2]["range"]["start"]["line"], 3);
            lenses
                .iter()
                .map(|l| l["command"]["title"].clone())
                .collect()
        };
        assert_eq!(
            titles(&mut server),
            ["env: ○ dev", "○ prod", "env: ○ dev", "○ prod"]
        );

        let lenses = server.handle(&request(2, "textDocument/codeLens", params.clone()));
        let pick_prod = lenses[0]["result"][1]["command"].clone();
        let replies = server.handle(&request(3, "workspace/executeCommand", pick_prod));
        assert_eq!(replies.len(), 2, "lens clicks only refresh the lenses");
        assert_eq!(replies[1]["method"], "workspace/codeLens/refresh");
        assert_eq!(
            titles(&mut server),
            ["env: ○ dev", "● prod", "env: ○ dev", "● prod"]
        );

        let file_arg = file.to_string_lossy();
        let replies = server.handle(&request(
            3,
            "workspace/executeCommand",
            json!({ "command": PICK, "arguments": [file_arg] }),
        ));
        let prompt = &replies[1];
        assert_eq!(prompt["method"], "window/showMessageRequest");
        assert_eq!(
            prompt["params"]["actions"],
            json!([{ "title": "dev" }, { "title": "prod" }, { "title": CLEAR_TITLE }])
        );
        let replies = server.handle(&json!({
            "jsonrpc": "2.0", "id": prompt["id"], "result": { "title": "prod" }
        }));
        assert_eq!(replies[0]["method"], "workspace/codeLens/refresh");

        let actions = server.handle(&request(4, "textDocument/codeAction", params.clone()));
        let result = actions[0]["result"].as_array().unwrap();
        assert_eq!(result[0]["title"], "HTTP: use environment \"dev\"");
        assert_eq!(result[1]["title"], "✓ HTTP environment: prod (current)");

        let replies = server.handle(&request(
            5,
            "workspace/executeCommand",
            result[0]["command"].clone(),
        ));
        assert_eq!(replies[1]["params"]["message"], "HTTP environment: dev");
        assert_eq!(replies[2]["method"], "workspace/codeLens/refresh");

        let replies = server.handle(&request(
            6,
            "workspace/executeCommand",
            json!({ "command": CLEAR, "arguments": [file.to_string_lossy()] }),
        ));
        assert_eq!(replies[1]["params"]["message"], "HTTP environment cleared");
        let envs = Environments::discover(&file).unwrap().unwrap();
        assert_eq!(envs.saved_selection(), None);
    }
}
