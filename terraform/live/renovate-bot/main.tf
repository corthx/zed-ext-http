terraform {
  required_version = ">=1.9, <2.0.0"

  backend "http" {}

  required_providers {
    gitlab = {
      source  = "gitlabhq/gitlab"
      version = "~> 19.3"
    }
  }
}

locals {
  gitlab_base_url = "https://gitlab.com"
}

provider "gitlab" {
  base_url = local.gitlab_base_url
}

module "bot" {
  source  = "gitlab.com/corthx/service-account/gitlab"
  version = "0.0.3"

  gitlab_base_url = local.gitlab_base_url
  project         = "corthx/zed-ext-http"
  name            = "Renovate Bot"
  username        = "zed-ext-http-renovate-bot"
  access_level    = "developer"
  token_name      = "Renovate Bot Token used in Zed HTTP Extension GitLab CI"
  # The scopes Renovate documents for the GitLab platform.
  token_scopes = ["api", "read_user", "write_repository"]

  token_rotate_before_days       = 14
  token_rotation_expiration_days = 180

  # Protected: only the renovate schedule, which runs on the default branch need these
  ci_variable_token_name        = "RENOVATE_TOKEN"
  ci_variable_token_protected   = true
  ci_variable_token_hidden      = true
  ci_variable_token_description = "Token used by Renovate to open dependency update MRs"

  ci_variable_email_name        = "RENOVATE_GIT_AUTHOR_EMAIL"
  ci_variable_email_protected   = true
  ci_variable_email_description = "Git author email of Renovate's commits, so GitLab attributes them to the bot"

  avatar_path = "${path.module}/avatar.png"
}
