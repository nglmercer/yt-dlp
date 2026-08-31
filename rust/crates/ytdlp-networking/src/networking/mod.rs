use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

include!("url.rs");
include!("headers.rs");
include!("cookies.rs");
include!("proxy.rs");
include!("model.rs");
include!("http.rs");
include!("reqwest.rs");
include!("parsing.rs");
include!("director.rs");
include!("tests.rs");
