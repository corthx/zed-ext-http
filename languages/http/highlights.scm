[
  (comment)
  (request_separator)
] @comment

(comment
  "@" @attribute
  name: (_) @attribute)

(method) @keyword

(request
  url: (_) @link_uri)

(http_version) @constant

(header
  name: (_) @property)

(header
  ":" @punctuation.delimiter)

(variable_declaration
  name: (identifier) @variable)

(variable_declaration
  "=" @operator)

[
  "{{"
  "}}"
] @punctuation.special

(variable
  name: (identifier) @variable.special)

(status_code) @number
(status_text) @string

(external_body
  path: (_) @string.special)

[
  (res_handler_script)
  (pre_request_script)
] @embedded
