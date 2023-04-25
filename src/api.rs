use std::{fmt::Debug, str::FromStr, sync::Arc};

use log::error;
use solana_program::pubkey::Pubkey;
use tiny_http::{Request, Response, Server};

use crate::{db::DB, filters::GeyserFilters, geyser_plugin_hook::GeyserError};
pub struct Api {
    server: Server,
    filters: Arc<GeyserFilters>,
}

impl Api {
    pub fn new(port: usize, filters: Arc<GeyserFilters>) -> Self {
        Self {
            server: Server::http(format!("0.0.0.0:{}", port)).unwrap(),
            filters,
        }
    }

    pub fn response_string<T: Debug>(&self, request: Request, data: T) {
        let response = Response::from_string(format!("{:?}", data));
        if request.respond(response).is_err() {
            error!("cannot send api response")
        }
    }

    pub fn response_with_code(&self, request: Request, code: u32) {
        let response = Response::empty(code);
        if request.respond(response).is_err() {
            error!("cannot send api response")
        }
    }

    pub fn start(&self, db: DB) {
        for mut request in self.server.incoming_requests() {
            match request.method() {
                tiny_http::Method::Get => {
                    let filters = self.filters.get_filters();
                    self.response_string(request, filters);
                }
                tiny_http::Method::Post => {
                    let mut content = String::new();

                    if request
                        .as_reader()
                        .read_to_string(&mut content)
                        .map_err(|e| error!("{}", e))
                        .is_ok()
                    {
                        if let Ok(pubkeys) = process_request(content)
                            .map_err(|e| error!("cannot process request: {:?}", e))
                        {
                            if self
                                .filters
                                .update_filters(pubkeys.clone())
                                .map_err(|e| error!("update_filters: {:?}", e))
                                .is_ok()
                                && db
                                    .set_filters(pubkeys.clone())
                                    .map_err(|e| error!("set_filters: {:?}", e))
                                    .is_ok()
                            {
                                self.response_string(request, pubkeys);
                                continue;
                            }
                        }
                    }

                    self.response_with_code(request, 502);
                }
                _ => self.response_with_code(request, 404),
            }
        }
    }
}

fn process_request(content: String) -> Result<Vec<Pubkey>, GeyserError<'static>> {
    let fr: Vec<String> = serde_json::from_str(&content)
        .map_err(|_| GeyserError::CustomError("cannot deserialize"))?;

    let x: Vec<Pubkey> = fr
        .iter()
        .map(|i| Pubkey::from_str(i))
        .filter(|i| i.is_ok())
        .map(|i| i.unwrap())
        .collect();

    if fr.len() != x.len() {
        return Err(GeyserError::CustomError("some of pubkeys are invalid"));
    }

    Ok(x)
}
