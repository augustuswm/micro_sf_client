extern crate serde_json;

use reqwest::{Client, Error as ClientError, RequestBuilder};

use std::cmp::PartialEq;
use std::collections::HashMap;
use std::io::Read;

#[derive(Debug)]
pub struct TokenRequest<'a, 'b, 'c, 'd, 'e, 'f> {
    login_url: &'a str,
    client_id: &'b str,
    client_secret: &'c str,
    username: &'d str,
    password: &'e str,
    client: &'f Client,
}

impl<'a, 'b, 'c, 'd, 'e, 'f> PartialEq for TokenRequest<'a, 'b, 'c, 'd, 'e, 'f> {
    fn eq(&self, other: &TokenRequest) -> bool {
        self.login_url == other.login_url && self.client_id == other.client_id &&
        self.client_secret == other.client_secret && self.username == other.username &&
        self.password == other.password
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TokenResponse {
    access_token: String,
    token_type: String,
    instance_url: String,
    signature: String,
    issued_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenErrorResponse {
    error: String,
    error_description: String,
}

#[derive(Debug)]
pub enum AuthFailure {
    InvalidClientId,
    InvalidClientSecret,
    InvalidGrant,
    InvalidUser,
    OrgUnavailable,
    RateLimitExceeded,
    TokenUnavailable,
}

impl<'a> From<&'a str> for AuthFailure {
    fn from(val: &'a str) -> AuthFailure {
        match val {
            "invalid_client_id" => AuthFailure::InvalidClientId,
            "invalid_client_credentials" => AuthFailure::InvalidClientSecret,
            "invalid_grant" => AuthFailure::InvalidGrant,
            "inactive_user" => AuthFailure::InvalidUser,
            "inactive_org" => AuthFailure::OrgUnavailable,
            "rate_limit_exceeded" => AuthFailure::RateLimitExceeded,
            _ => AuthFailure::TokenUnavailable,
        }
    }
}

#[derive(Debug)]
pub enum TokenError {
    AuthResponseParseFailure,
    APIError(AuthFailure),
    Network(ClientError),
}

pub type TokenResult = Result<TokenResponse, TokenError>;

impl<'a, 'b, 'c, 'd, 'e, 'f> TokenRequest<'a, 'b, 'c, 'd, 'e, 'f> {
    pub fn new(login_url: &'a str,
               client_id: &'b str,
               client_secret: &'c str,
               username: &'d str,
               password: &'e str,
               client: &'f Client)
               -> TokenRequest<'a, 'b, 'c, 'd, 'e, 'f> {
        TokenRequest {
            login_url: login_url,
            client_id: client_id,
            client_secret: client_secret,
            username: username,
            password: password,
            client: client,
        }
    }

    fn build_request(&self) -> RequestBuilder {
        let mut auth_params = HashMap::new();
        auth_params.insert("grant_type", "password");
        auth_params.insert("client_id", self.client_id);
        auth_params.insert("client_secret", self.client_secret);
        auth_params.insert("username", self.username);
        auth_params.insert("password", self.password);

        self.client.post(self.login_url).form(&auth_params)
    }

    pub fn send(&self) -> TokenResult {
        let mut response = self.build_request().send().map_err(TokenError::Network)?;

        let mut content = String::new();
        response.read_to_string(&mut content);

        if let Ok(token) = serde_json::from_str::<TokenResponse>(content.as_str()) {
            Ok(token)
        } else if let Ok(token_error) =
            serde_json::from_str::<TokenErrorResponse>(content.as_str()) {
            Err(TokenError::APIError(AuthFailure::from(token_error.error.as_str())))
        } else {
            Err(TokenError::AuthResponseParseFailure)
        }
    }
}

impl TokenResponse {
    pub fn new(access_token: &str,
               token_type: &str,
               instance_url: &str,
               signature: &str,
               issued_at: &str)
               -> TokenResponse {
        TokenResponse {
            access_token: access_token.to_string(),
            token_type: token_type.to_string(),
            instance_url: instance_url.to_string(),
            signature: signature.to_string(),
            issued_at: issued_at.to_string(),
        }
    }

    pub fn url(&self) -> &str {
        self.instance_url.as_str()
    }

    pub fn access(&self) -> &str {
        self.access_token.as_str()
    }
}

#[cfg(test)]
mod tests {
    use mockito;
    use mockito::{mock, Mock};
    use reqwest::Client;
    use serde_json;

    use token::AuthFailure;
    use token::TokenError;
    use token::TokenRequest;
    use token::TokenResponse;

    const ACCESS: &'static str = "00Dx0000000BV7z!AR8AQAxo9UfVkh8AlV0Gomt9Czx9LjHnSSpwBMmbRcgKFmxOtvxjTrKW19ye6PE3Ds1eQz3z8jr3W7_VbWmEu4Q8TVGSTHxs";

    macro_rules! auth_client {
        ( $login_url:expr, $client:expr ) => {
            TokenRequest::new(
                $login_url.as_str(),
                "id",
                "secret",
                "user",
                "pass",
                $client
            )
        }
    }

    fn auth_path(path: &str) -> String {
        "/mock_auth_url/".to_owned() + path
    }

    fn auth_url(path: &str) -> String {
        mockito::SERVER_URL.to_owned() + auth_path(path).as_str()
    }

    fn auth_mock(url: String, code: usize, body: String) -> Mock {
        let mut m = mock("POST", url.as_str());
        m.with_status(code)
            .with_body(body.as_str())
            .match_header("content-type", "application/x-www-form-urlencoded");
        m.create();
        m
    }

    fn auth_err(err: &str) -> String {
        let resp = json!({
            "error": err,
            "error_description": "mock error"
        });

        resp.to_string()
    }

    fn auth_success() -> String {
        let resp = json!({
            "id": mockito::SERVER_URL.to_owned() + "/id/",
            "issued_at": "1278448832702",
            "instance_url": mockito::SERVER_URL.to_owned() + "/instance/",
            "signature": "0CmxinZir53Yex7nE0TD+zMpvIWYGb/bdJh6XfOH6EQ=",
            "access_token": ACCESS,
            "token_type": "Bearer"
        });

        resp.to_string()
    }

    macro_rules! auth_fail_test {
        ( $error:expr, $error_value:pat, $error_msg:expr ) => {
            let client = Client::new().unwrap();
            let path = auth_path($error);
            let error = auth_err($error);
            let url = auth_url($error);
            let mock = auth_mock(path, 200, error);
            let auth = auth_client!(url, &client);

            match auth.send() {
                $error_value => (),
                _ => panic!($error_msg),
            }

            mock.remove();
        }
    }

    #[test]
    fn test_auth_parses_token() {
        let client = Client::new().unwrap();
        let token = serde_json::from_str::<TokenResponse>(auth_success().as_str()).unwrap();
        let path = auth_path("auth_success");
        let url = auth_url("auth_success");
        let mock = auth_mock(path, 200, auth_success());
        let auth = auth_client!(url, &client);

        assert_eq!(auth.send().unwrap(), token);

        mock.remove();
    }

    #[test]
    fn test_auth_handles_invalid_client_id() {
        auth_fail_test!(
            "invalid_client_id",
            Err(TokenError::APIError(AuthFailure::InvalidClientId)),
            "Failed to handle invalid_client_id"
        );
    }

    #[test]
    fn test_auth_handles_invalid_client_secret() {
        auth_fail_test!(
            "invalid_client_credentials",
            Err(TokenError::APIError(AuthFailure::InvalidClientSecret)),
            "Failed to handle invalid_client_credentials"
        );
    }

    #[test]
    fn test_auth_handles_invalid_grant() {
        auth_fail_test!(
            "invalid_grant",
            Err(TokenError::APIError(AuthFailure::InvalidGrant)),
            "Failed to handle invalid_grant"
        );
    }

    #[test]
    fn test_auth_handles_inactive_user() {
        auth_fail_test!(
            "inactive_user",
            Err(TokenError::APIError(AuthFailure::InvalidUser)),
            "Failed to handle inactive_user"
        );
    }

    #[test]
    fn test_auth_handles_inactive_org() {
        auth_fail_test!(
            "inactive_org",
            Err(TokenError::APIError(AuthFailure::OrgUnavailable)),
            "Failed to handle inactive_org"
        );
    }

    #[test]
    fn test_auth_handles_rate_limit_exceeded() {
        auth_fail_test!(
            "rate_limit_exceeded",
            Err(TokenError::APIError(AuthFailure::RateLimitExceeded)),
            "Failed to handle rate_limit_exceeded"
        );
    }
}
