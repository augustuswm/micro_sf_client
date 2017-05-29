extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use reqwest::{Client, Error as ClientError, RequestBuilder, StatusCode};
use reqwest::header::Authorization;

use std::collections::HashMap;

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
struct Token {
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

pub struct QueryResponse {}

impl SFClient {
    pub fn new(login_url: &str,
               client_id: &str,
               client_secret: &str,
               username: &str,
               password: &str)
               -> SFClientResult<SFClient> {

        if login_url == "" {
            return Err(SFClientError::InvalidLoginUrl);
        }

        Client::new()
            .map(|client| {
                SFClient {
                    login_url: login_url.to_string(),
                    client_id: client_id.to_string(),
                    client_secret: client_secret.to_string(),
                    username: username.to_string(),
                    password: password.to_string(),
                    client: client,
                    attempt_limit: 3,
                    token: None,
                }
            })
            .map_err(SFClientError::ClientBuildFailure)
    }

    pub fn attempt_limit(mut self, attempt_limit: u8) -> SFClient {
        self.attempt_limit = attempt_limit;
        self
    }

    fn authenticate(&mut self) -> SFClientResult<()> {
        let auth = {
            let mut auth_params = HashMap::new();
            auth_params.insert("grant_type", "password");
            auth_params.insert("client_id", self.client_id.as_str());
            auth_params.insert("client_secret", self.client_secret.as_str());
            auth_params.insert("username", self.username.as_str());
            auth_params.insert("password", self.password.as_str());

            self.client
                .post(self.login_url.as_str())
                .form(&auth_params)
                .send()
        };

        let mut response = auth.map_err(SFClientError::Network)?;

        if let Ok(token) = response.json::<Token>() {
            self.token = Some(token);
            Ok(())
        } else if let Ok(token_error) = response.json::<TokenError>() {
            Err(match token_error.error_description.as_str() {
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
            let url = token.instance_url.to_owned() + "query?q=" + query;

            Ok(self.client
                   .get(url.as_str())
                   .header(Authorization(token.access_token.clone())))
        } else {
            Err(SFClientError::TokenUnavailable)
        }
    }

    fn do_query(&mut self, query: &str) -> SFClientResult<QueryResponse> {
        self.build_request(query)
            .and_then(|request| request.send().map_err(SFClientError::Network))
            .and_then(|response| match *response.status() {
                          StatusCode::Ok => Ok(QueryResponse {}),
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
    pub fn new(access_token: String,
               token_type: String,
               instance_url: String,
               signature: String,
               issued_at: String)
               -> Token {
        Token {
            access_token: access_token,
            token_type: token_type,
            instance_url: instance_url,
            signature: signature,
            issued_at: issued_at,
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
    Network(ClientError),
}

#[cfg(test)]
mod tests {

    use SFClient;
    use SFClientError;

    #[test]
    fn test_requires_login_url() {
        match SFClient::new("", "c_id", "c_secret", "user", "pass") {
            Err(SFClientError::InvalidLoginUrl) => (),
            _ => panic!("Failed to detect empty login url"),
        };
    }

    #[test]
    fn test_auth_handles_invalid_client_id() {
        // "invalid_client_id" => SFClientError::InvalidClientId,
        unimplemented!()
    }

    #[test]
    fn test_auth_handles_invalid_client_secret() {
        // "invalid_client_credentials" => SFClientError::InvalidClientSecret,
        unimplemented!()
    }

    #[test]
    fn test_auth_handles_invalid_grant() {
        // "invalid_grant" => SFClientError::InvalidGrant,
        unimplemented!()
    }

    #[test]
    fn test_auth_handles_inactive_user() {
        // "inactive_user" => SFClientError::InvalidUser,
        unimplemented!()
    }

    #[test]
    fn test_auth_handles_inactive_org() {
        // "inactive_org" => SFClientError::OrgUnavailable,
        unimplemented!()
    }

    #[test]
    fn test_auth_handles_rate_limit_exceeded() {
        // "rate_limit_exceeded" => SFClientError::RateLimitExceeded,
        unimplemented!()
    }

    #[test]
    fn test_authenticates_without_token() {
        unimplemented!()
    }

    #[test]
    fn test_reauthenticates_with_expired_token() {
        unimplemented!()
    }

    #[test]
    fn test_retries_to_limit() {
        unimplemented!()
    }

    #[test]
    fn test_calls_query_endpoint() {
        unimplemented!()
    }

    #[test]
    fn test_token_access() {
        unimplemented!()
    }

    #[test]
    fn test_token_url() {
        unimplemented!()
    }
}
