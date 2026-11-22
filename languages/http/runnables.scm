; @url is $ZED_CUSTOM_url in tasks.json.
(request
  url: (target_url) @run @url
  (#set! tag http-request))
