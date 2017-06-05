use reqwest::{Client, Error as ClientError, RequestBuilder, StatusCode};
use reqwest::header::{Authorization, Bearer};
use serde_json::Value;

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

#[derive(Debug)]
pub enum QueryError {
    API(QueryFailure),
    QueryResponseParseFailure,
    Network(ClientError),
}

pub type QueryResult = Result<QueryResponse, QueryError>;

impl<'a, 'b, 'c, 'd, 'e> QueryRequest<'a, 'b, 'c, 'd, 'e> {
    pub fn new(endpoint: &'a str,
               version: &'b str,
               query: &'c str,
               token: &'d str,
               client: &'e Client)
               -> QueryRequest<'a, 'b, 'c, 'd, 'e> {
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
        self.client
            .get(url.as_str())
            .header(Authorization(Bearer { token: self.token.to_string() }))

    }

    pub fn send(&self) -> QueryResult {
        self.build_request()
            .send()
            .map_err(QueryError::Network)
            .and_then(|mut response| match *response.status() {
                          StatusCode::Ok => {
                              response
                                  .json::<QueryResponse>()
                                  .or_else(|_| Err(QueryError::QueryResponseParseFailure))
                          }
                          error_code => {
                              let mut error =
                                  response
                                      .json::<QueryFailure>()
                                      .or_else(|_| Err(QueryError::QueryResponseParseFailure))?;

                              error.error_code = error_code.to_u16();

                              Err(QueryError::API(error))
                          }
                      })
    }
}
