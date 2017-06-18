#[cfg(test)]
extern crate mockito;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

mod query;
mod token;

use std::error::Error;
use std::fmt;

use reqwest::{Client, Error as ClientError};

use query::{QueryError, QueryRequest, QueryResponse};
use token::{TokenError, TokenRequest, TokenResponse};

#[derive(Debug)]
pub struct SFClient {
    login_url: String,
    version: String,
    client_id: String,
    client_secret: String,
    username: String,
    password: String,
    client: Client,
    attempt_limit: u8,
    token: Option<TokenResponse>,
}

impl SFClient {
    pub fn new<S: Into<String>>(
        login_url: S,
        version: S,
        client_id: S,
        client_secret: S,
        username: S,
        password: S,
    ) -> SFClientResult<SFClient> {

        let url = login_url.into();

        if url == "" {
            return Err(SFClientError::InvalidLoginUrl);
        }

        let api_version = version.into();

        if api_version == "" {
            return Err(SFClientError::InvalidVersion);
        }

        Client::new()
            .map(|client| {
                SFClient {
                    login_url: url,
                    version: api_version,
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

    pub fn set_token(&mut self, token: TokenResponse) {
        self.token = Some(token);
    }

    pub fn token(&self) -> Option<&TokenResponse> {
        match self.token {
            Some(ref t) => Some(&t),
            None => None,
        }
    }

    fn authenticate(&mut self) -> SFClientResult<()> {
        let request = TokenRequest::new(
            self.login_url.as_str(),
            self.client_id.as_str(),
            self.client_secret.as_str(),
            self.username.as_str(),
            self.password.as_str(),
            &self.client,
        );

        let token_resp = request.send();
        let token = token_resp.map_err(SFClientError::Token)?;
        self.token = Some(token);

        Ok(())
    }

    fn build_request<'a, 'b>(
        &'a mut self,
        query: &'b str,
    ) -> SFClientResult<QueryRequest<'a, 'a, 'b, 'a, 'a>> {
        if self.token.is_none() {
            self.authenticate()?;
        };

        if let Some(ref token) = self.token {
            Ok(QueryRequest::new(
                token.url(),
                self.version.as_str(),
                query,
                token.access(),
                &self.client,
            ))
        } else {
            Err(SFClientError::TokenUnavailable)
        }
    }

    fn do_query(&mut self, query: &str) -> SFClientResult<QueryResponse> {
        self.build_request(query).and_then(|request| {
            request.send().map_err(|failure| match failure {
                QueryError::Network(net_failure) => SFClientError::Network(net_failure),
                error => SFClientError::Query(error),
            })
        })
    }

    fn attempt_query(&mut self, query: &str, attempt: u8) -> SFClientResult<QueryResponse> {
        self.do_query(query).or_else(
            |err| if attempt < self.attempt_limit {
                if let SFClientError::Query(QueryError::API(failure)) = err {
                    if failure.error_code == 401 {
                        self.token = None;
                    }
                }

                self.attempt_query(query, attempt + 1)
            } else {
                Err(err)
            },
        )
    }

    pub fn query(&mut self, query: &str) -> SFClientResult<QueryResponse> {
        self.attempt_query(query, 0)
    }
}

pub type SFClientResult<T> = Result<T, SFClientError>;

#[derive(Debug)]
pub enum SFClientError {
    InvalidLoginUrl,
    InvalidVersion,
    ClientBuildFailure(ClientError),
    Token(TokenError),
    Query(QueryError),
    TokenUnavailable,
    Network(ClientError),
}

impl fmt::Display for SFClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SFClientError::InvalidLoginUrl => {
                write!(f, "Supplied login url is not a valid login url")
            }
            SFClientError::InvalidVersion => {
                write!(f, "Supplied version is not a valid API version")
            }
            SFClientError::ClientBuildFailure(ref err) => err.fmt(f),
            SFClientError::Token(ref err) => err.fmt(f),
            SFClientError::Query(ref err) => err.fmt(f),
            SFClientError::TokenUnavailable => write!(f, "Failed to get token from the API"),
            SFClientError::Network(ref err) => err.fmt(f),
        }
    }
}

impl Error for SFClientError {
    fn description(&self) -> &str {
        match *self {
            SFClientError::InvalidLoginUrl => "Supplied login url is not a valid login url",
            SFClientError::InvalidVersion => "Supplied version is not a valid API version",
            SFClientError::ClientBuildFailure(ref err) => err.description(),
            SFClientError::Token(ref err) => err.description(),
            SFClientError::Query(ref err) => err.description(),
            SFClientError::TokenUnavailable => "Failed to get token from the API",
            SFClientError::Network(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            SFClientError::InvalidLoginUrl => None,
            SFClientError::InvalidVersion => None,
            SFClientError::ClientBuildFailure(ref err) => Some(err),
            SFClientError::Token(ref err) => Some(err),
            SFClientError::Query(ref err) => Some(err),
            SFClientError::TokenUnavailable => None,
            SFClientError::Network(ref err) => Some(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use mockito;
    use mockito::{mock, Mock};
    use serde_json;

    use SFClient;
    use SFClientError;
    use query::{API_BASE, QueryResponse};
    use token::TokenResponse;

    const ACCESS: &'static str = "00Dx0000000BV7z!AR8AQAxo9UfVkh8AlV0Gomt9Czx9LjHnSSpwBMmbRcgKFmxOtvxjTrKW19ye6PE3Ds1eQz3z8jr3W7_VbWmEu4Q8TVGSTHxs";

    macro_rules! test_client {
        ( $login_url:expr, $attempts:expr ) => {{
            let mut client = SFClient::new(
                $login_url.as_str(),
                "v20.0",
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
        m.with_status(code).with_body(body.as_str()).match_header(
            "content-type",
            "application/x-www-form-urlencoded",
        );
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

    fn query_path(path: &str, version: &str) -> String {
        "/instance/".to_owned() + API_BASE + version + "/query?q=" + path
    }

    fn query_mock(url: String, code: usize, body: String, token: &str) -> Mock {
        let mut m = mock("GET", url.as_str());
        let auth_header = "Bearer ".to_owned() + token;
        m.with_status(code).with_body(body.as_str()).match_header(
            "Authorization",
            auth_header
                .as_str(),
        );
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
            "fields": [],
            "message": "Token is expired"
        });

        resp.to_string()
    }

    #[test]
    fn test_requires_login_url() {
        match SFClient::new("", "v20.0", "c_id", "c_secret", "user", "pass") {
            Err(SFClientError::InvalidLoginUrl) => (),
            _ => panic!("Failed to detect empty login url"),
        };
    }

    #[test]
    fn test_requires_version() {
        match SFClient::new("http://127.0.0.1", "", "c_id", "c_secret", "user", "pass") {
            Err(SFClientError::InvalidVersion) => (),
            _ => panic!("Failed to detect empty version"),
        };
    }

    #[test]
    fn test_authenticates_without_token() {
        let a_mock = auth_mock(auth_path("without_token"), 200, auth_success());
        let q_mock = query_mock(
            query_path("without_token", "v20.0"),
            200,
            query_success(),
            ACCESS,
        );
        let mut client = test_client!(auth_url("without_token"), 0);

        client.query("without_token");

        a_mock.remove();
        q_mock.remove();

        assert_eq!(ACCESS, client.token().unwrap().access());
        assert_eq!("http://127.0.0.1:1234/instance/", client.token().unwrap().url());
    }

    #[test]
    fn test_reauthenticates_with_invalid_token() {
        let a_mock = auth_mock(auth_path("invalid_token"), 200, auth_success());
        let q_mock = query_mock(
            query_path("invalid_token", "v20.0"),
            401,
            query_error(),
            "invalid",
        );
        let mut client = test_client!(auth_url("invalid_token"), 1);

        let instance_url = mockito::SERVER_URL.to_owned() + "/instance/";
        client.set_token(TokenResponse::new(
            "invalid",
            "",
            instance_url.as_str(),
            "",
            "",
        ));
        client.query("invalid_token");

        a_mock.remove();
        q_mock.remove();

        assert_eq!(ACCESS, client.token().unwrap().access());
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
        let a_mock = auth_mock(auth_path("query_test"), 200, auth_success());
        let q_mock = query_mock(
            query_path("query_test", "v20.0"),
            200,
            query_success(),
            ACCESS,
        );
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
