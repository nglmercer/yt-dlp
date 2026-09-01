use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use url::Url;
use yt_dlp_networking::{ErrorKind, Request, RequestDirector, RequestError};

include!("core.rs");
include!("direct.rs");
include!("hls.rs");
include!("dash.rs");
include!("segment.rs");
include!("output.rs");
include!("tests.rs");
