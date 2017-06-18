use reqwest::{Client, Error as ClientError, RequestBuilder, StatusCode};
use reqwest::header::{Authorization, Bearer};
use serde_json::Value;

use std::error::Error;
use std::fmt;

pub static API_BASE: &'static str = "services/data/";

#[derive(Debug)]
pub struct QueryRequest<'a, 'b, 'c, 'd, 'e> {
    endpoint: &'a str,
    version: &'b str,
    query: &'c str,
    token: &'d str,
    client: &'e Client,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct QueryResponse {
    total_size: u8,
    done: bool,
    records: Vec<Value>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct QueryFailure {
    pub message: String,
    #[serde(skip_deserializing)]
    pub error_code: u16,
    pub fields: Vec<String>,
}

impl fmt::Display for QueryFailure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Error [{}] {} : {:?}",
            self.error_code,
            self.message,
            self.fields
        )
    }
}

impl<'a, 'b, 'c, 'd, 'e> QueryRequest<'a, 'b, 'c, 'd, 'e> {
    pub fn new(
        endpoint: &'a str,
        version: &'b str,
        query: &'c str,
        token: &'d str,
        client: &'e Client,
    ) -> QueryRequest<'a, 'b, 'c, 'd, 'e> {
        QueryRequest {
            endpoint: endpoint,
            version: version,
            query: query,
            token: token,
            client: client,
        }
    }

    fn build_request(&self) -> RequestBuilder {
        let url = self.endpoint.to_owned() + API_BASE + self.version + "/query?q=" + self.query;
        self.client.get(url.as_str()).header(Authorization(Bearer {
            token: self.token.to_string(),
        }))

    }

    pub fn send(&self) -> QueryResult {
        self.build_request()
            .send()
            .map_err(QueryError::Network)
            .and_then(|mut response| match *response.status() {
                StatusCode::Ok => {
                    response.json::<QueryResponse>().or_else(|_| {
                        Err(QueryError::QueryResponseParseFailure)
                    })
                }
                error_code => {
                    let mut error = response.json::<QueryFailure>().or_else(|_| {
                        Err(QueryError::QueryResponseParseFailure)
                    })?;

                    error.error_code = error_code.to_u16();

                    Err(QueryError::API(error))
                }
            })
    }
}

#[derive(Debug)]
pub enum QueryError {
    API(QueryFailure),
    QueryResponseParseFailure,
    Network(ClientError),
}

pub type QueryResult = Result<QueryResponse, QueryError>;

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            QueryError::QueryResponseParseFailure => {
                write!(f, "Failed to parse the query response from the API")
            }
            QueryError::API(ref failure) => write!(f, "{}", failure),
            QueryError::Network(ref err) => err.fmt(f),
        }
    }
}

impl Error for QueryError {
    fn description(&self) -> &str {
        match *self {
            QueryError::QueryResponseParseFailure => "query_response_parse_failed",
            QueryError::API(_) => "api_query_failure",
            QueryError::Network(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            QueryError::Network(ref err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use mockito;
    use mockito::{mock, Mock};
    use reqwest::Client;
    use serde_json;

    use QueryRequest;
    use QueryResponse;

    const API_BASE: &'static str = "services/data/";
    const VERSION: &'static str = "vXY.Z";
    const ACCESS: &'static str = "test-token";

    fn mock_path(query: &str) -> String {
        "/".to_owned() + API_BASE + VERSION + "/query?q=" + query
    }

    fn query_mock(url: String, code: usize, body: String) -> Mock {
        let mut m = mock("GET", url.as_str());
        let auth_header = "Bearer ".to_owned() + ACCESS;
        m.with_status(code).with_body(body.as_str()).match_header(
            "Authorization",
            auth_header
                .as_str(),
        );
        m.create();
        m
    }

    #[test]
    fn test_handles_successful_query() {
        let client = Client::new().unwrap();
        let ep = mockito::SERVER_URL.to_owned() + "/";
        let query = "query_success";
        let resp = QueryResponse {
            total_size: 1,
            done: true,
            records: vec![json!({"id": "12345"})],
        };
        let success = json!({
            "total_size": 1,
            "done": true,
            "records": [
                {"id": "12345"}
            ]
        });

        let mock = query_mock(mock_path(query), 200, success.to_string());
        let req = QueryRequest::new(ep.as_str(), VERSION, query, ACCESS, &client);

        assert_eq!(resp, req.send().unwrap());
    }
}
