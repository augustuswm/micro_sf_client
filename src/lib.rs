#[cfg(test)]
extern crate mockito;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use reqwest::{Client, Error as ClientError, RequestBuilder, StatusCode};
use reqwest::header::{Authorization, Bearer};
use serde_json::map::Map;
use serde_json::Value;

use std::collections::HashMap;
use std::io::Read;

static API_BASE: &'static str = "services/data/v20.0/";

#[derive(Debug)]
pub struct SFClient {
    login_url: String,
    client_id: String,
    client_secret: String,
    username: String,
    password: String,
    client: Client,
    attempt_limit: u8,
    token: Option<Token>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    access_token: String,
    token_type: String,
    instance_url: String,
    signature: String,
    issued_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenError {
    error: String,
    error_description: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct QueryResponse {
    total_size: u8,
    done: bool,
    records: Vec<Value>,
}

impl SFClient {
    pub fn new<S: Into<String>>(login_url: S,
                                client_id: S,
                                client_secret: S,
                                username: S,
                                password: S)
                                -> SFClientResult<SFClient> {

        let url = login_url.into();

        if url == "" {
            return Err(SFClientError::InvalidLoginUrl);
        }

        Client::new()
            .map(|client| {
                SFClient {
                    login_url: url,
                    client_id: client_id.into(),
                    client_secret: client_secret.into(),
                    username: username.into(),
                    password: password.into(),
                    client: client,
                    attempt_limit: 3,
                    token: None,
                }
            })
            .map_err(SFClientError::ClientBuildFailure)
    }

    pub fn set_attempt_limit(&mut self, attempt_limit: u8) {
        self.attempt_limit = attempt_limit;
    }

    pub fn set_token(&mut self, token: Token) {
        self.token = Some(token);
    }

    pub fn token(&self) -> Option<&Token> {
        match self.token {
            Some(ref t) => Some(&t),
            None => None,
        }
    }

    fn build_auth_request(&mut self) -> RequestBuilder {
        let mut auth_params = HashMap::new();
        auth_params.insert("grant_type", "password");
        auth_params.insert("client_id", self.client_id.as_str());
        auth_params.insert("client_secret", self.client_secret.as_str());
        auth_params.insert("username", self.username.as_str());
        auth_params.insert("password", self.password.as_str());

        self.client.post(self.login_url.as_str()).form(&auth_params)
    }

    fn authenticate(&mut self) -> SFClientResult<()> {
        let mut response = self.build_auth_request()
            .send()
            .map_err(SFClientError::Network)?;

        let mut content = String::new();
        response.read_to_string(&mut content);

        if let Ok(token) = serde_json::from_str::<Token>(content.as_str()) {
            self.token = Some(token);
            Ok(())
        } else if let Ok(token_error) = serde_json::from_str::<TokenError>(content.as_str()) {
            Err(match token_error.error.as_str() {
                    "invalid_client_id" => SFClientError::InvalidClientId,
                    "invalid_client_credentials" => SFClientError::InvalidClientSecret,
                    "invalid_grant" => SFClientError::InvalidGrant,
                    "inactive_user" => SFClientError::InvalidUser,
                    "inactive_org" => SFClientError::OrgUnavailable,
                    "rate_limit_exceeded" => SFClientError::RateLimitExceeded,
                    _ => SFClientError::TokenUnavailable,
                })
        } else {
            Err(SFClientError::TokenUnavailable)
        }
    }

    fn build_request(&mut self, query: &str) -> SFClientResult<RequestBuilder> {
        if self.token.is_none() {
            self.authenticate()?;
        };

        if let Some(ref token) = self.token {
            let url = token.instance_url.to_owned() + API_BASE + "query?q=" + query;

            Ok(self.client
                   .get(url.as_str())
                   .header(Authorization(Bearer { token: token.access_token.to_string() })))
        } else {
            Err(SFClientError::TokenUnavailable)
        }
    }

    fn do_query(&mut self, query: &str) -> SFClientResult<QueryResponse> {
        self.build_request(query)
            .and_then(|request| request.send().map_err(SFClientError::Network))
            .and_then(|mut response| match *response.status() {
                          StatusCode::Ok => {
                              response
                                  .json::<QueryResponse>()
                                  .or_else(|_| Err(SFClientError::QueryResponseParseFailure))
                          }
                          StatusCode::Unauthorized => {
                              self.token = None;
                              Err(SFClientError::QueryFailure)
                          }
                          _ => Err(SFClientError::QueryFailure),
                      })
    }

    fn attempt_query(&mut self, query: &str, attempt: u8) -> SFClientResult<QueryResponse> {
        self.do_query(query)
            .or_else(|err| if attempt < self.attempt_limit {
                         self.attempt_query(query, attempt + 1)
                     } else {
                         Err(err)
                     })
    }

    pub fn query(&mut self, query: &str) -> SFClientResult<QueryResponse> {
        self.attempt_query(query, 0)
    }
}

impl Token {
    pub fn new<S: Into<String>>(access_token: S,
                                token_type: S,
                                instance_url: S,
                                signature: S,
                                issued_at: S)
                                -> Token {
        Token {
            access_token: access_token.into(),
            token_type: token_type.into(),
            instance_url: instance_url.into(),
            signature: signature.into(),
            issued_at: issued_at.into(),
        }
    }

    pub fn token(&self) -> &str {
        self.access_token.as_str()
    }

    pub fn url(&self) -> &str {
        self.instance_url.as_str()
    }
}

pub type SFClientResult<T> = Result<T, SFClientError>;

#[derive(Debug)]
pub enum SFClientError {
    InvalidLoginUrl,
    ClientBuildFailure(ClientError),
    TokenUnavailable,
    InvalidClientId,
    InvalidClientSecret,
    InvalidGrant,
    InvalidUser,
    OrgUnavailable,
    RateLimitExceeded,
    QueryFailure,
    QueryResponseParseFailure,
    Network(ClientError),
}

#[cfg(test)]
mod tests {
    use mockito;
    use mockito::{mock, Mock};
    use serde_json;

    use API_BASE;
    use QueryResponse;
    use SFClient;
    use SFClientError;
    use Token;

    const ACCESS: &'static str = "00Dx0000000BV7z!AR8AQAxo9UfVkh8AlV0Gomt9Czx9LjHnSSpwBMmbRcgKFmxOtvxjTrKW19ye6PE3Ds1eQz3z8jr3W7_VbWmEu4Q8TVGSTHxs";

    macro_rules! test_client {
        ( $login_url:expr, $attempts:expr ) => {{
            let mut client = SFClient::new(
                $login_url.as_str(),
                "id",
                "secret",
                "user",
                "pass"
            ).unwrap();
            client.set_attempt_limit($attempts);
            client
        }}
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
            let mut mock = auth_mock(auth_path($error), 200, auth_err($error));
            let mut client = test_client!(auth_url($error), 0);

            match client.query("") {
                $error_value => (),
                _ => panic!($error_msg),
            }

            mock.remove();
        }
    }

    fn query_path(path: &str) -> String {
        "/instance/".to_owned() + API_BASE + "query?q=" + path
    }

    fn query_mock(url: String, code: usize, body: String, token: &str) -> Mock {
        let mut m = mock("GET", url.as_str());
        let auth_header = "Bearer ".to_owned() + token;
        m.with_status(code)
            .with_body(body.as_str())
            .match_header("Authorization", auth_header.as_str());
        m.create();
        m
    }

    fn query_success() -> String {
        let resp = json!({
            "total_size": 1,
            "done": true,
            "records": [
                {"id": "12345"}
            ]
        });

        resp.to_string()
    }

    fn query_error() -> String {
        let resp = json!({
            "error": "expired_token"
        });

        resp.to_string()
    }

    #[test]
    fn test_requires_login_url() {
        match SFClient::new("", "c_id", "c_secret", "user", "pass") {
            Err(SFClientError::InvalidLoginUrl) => (),
            _ => panic!("Failed to detect empty login url"),
        };
    }

    #[test]
    fn test_auth_handles_invalid_client_id() {
        auth_fail_test!(
            "invalid_client_id",
            Err(SFClientError::InvalidClientId),
            "Failed to handle invalid_client_id"
        );
    }

    #[test]
    fn test_auth_handles_invalid_client_secret() {
        auth_fail_test!(
            "invalid_client_credentials",
            Err(SFClientError::InvalidClientSecret),
            "Failed to handle invalid_client_credentials"
        );
    }

    #[test]
    fn test_auth_handles_invalid_grant() {
        auth_fail_test!(
            "invalid_grant",
            Err(SFClientError::InvalidGrant),
            "Failed to handle invalid_grant"
        );
    }

    #[test]
    fn test_auth_handles_inactive_user() {
        auth_fail_test!(
            "inactive_user",
            Err(SFClientError::InvalidUser),
            "Failed to handle inactive_user"
        );
    }

    #[test]
    fn test_auth_handles_inactive_org() {
        auth_fail_test!(
            "inactive_org",
            Err(SFClientError::OrgUnavailable),
            "Failed to handle inactive_org"
        );
    }

    #[test]
    fn test_auth_handles_rate_limit_exceeded() {
        auth_fail_test!(
            "rate_limit_exceeded",
            Err(SFClientError::RateLimitExceeded),
            "Failed to handle rate_limit_exceeded"
        );
    }

    #[test]
    fn test_authenticates_without_token() {
        let mut a_mock = auth_mock(auth_path("without_token"), 200, auth_success());
        let mut q_mock = query_mock(query_path("without_token"), 200, query_success(), ACCESS);
        let mut client = test_client!(auth_url("without_token"), 0);

        client.query("without_token");

        a_mock.remove();
        q_mock.remove();

        assert_eq!(ACCESS, client.token().unwrap().token());
        assert_eq!("http://127.0.0.1:1234/instance/", client.token.unwrap().url());
    }

    #[test]
    fn test_reauthenticates_with_invalid_token() {
        let mut a_mock = auth_mock(auth_path("invalid_token"), 200, auth_success());
        let mut q_mock = query_mock(query_path("invalid_token"), 401, query_error(), "invalid");
        let mut client = test_client!(auth_url("invalid_token"), 1);

        let instance_url = mockito::SERVER_URL.to_owned() + "/instance/";
        client.set_token(Token::new("invalid", "", instance_url.as_str(), "", ""));
        client.query("invalid_token");

        a_mock.remove();
        q_mock.remove();

        assert_eq!(ACCESS, client.token().unwrap().token());
        assert_eq!("http://127.0.0.1:1234/instance/", client.token().unwrap().url());
    }

    #[test]
    fn test_retries_to_limit() {
        let retries = 5;

        let mut a_mock = auth_mock(auth_path("test_retries"), 200, auth_err("invalid_grant"));
        a_mock.expect(retries + 1);

        let mut client = test_client!(auth_url("test_retries"), retries as u8);
        client.query("test_retries");

        a_mock.assert();
        a_mock.remove();
    }

    #[test]
    fn test_calls_query() {
        let mut a_mock = auth_mock(auth_path("query_test"), 200, auth_success());
        let mut q_mock = query_mock(query_path("query_test"), 200, query_success(), ACCESS);
        let mut client = test_client!(auth_url("query_test"), 0);

        let res = client.query("query_test");

        a_mock.remove();
        q_mock.remove();

        match res {
            Ok(result) => {
                assert_eq!(serde_json::from_str::<QueryResponse>(query_success().as_str()).unwrap(), result)
            }
            Err(err) => panic!("Query call test failed {:?}", err),
        };
    }
}
