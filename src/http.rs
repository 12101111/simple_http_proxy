use std::fmt;
// A module to parse HTTP request

// HTTP Header
pub struct Header<'a> {
    // In Rust, str is a slice of String
    // String is valid UTF-8
    key: &'a str,

    // colon storage ": "
    colon: &'a str,

    // Notice that specification allows for values that may not be
    // valid ASCII, nor UTF-8
    value: &'a [u8],
}

pub struct Request<'a> {
    // "GET" or "POST"
    pub method: &'a str,

    // path is String,so that it can be modified
    pub path: String,

    // "HTTP/1.1"
    pub version: &'a str,

    // TODO: maybe HashMap is better
    pub headers: Vec<Header<'a>>,

    // easy way to get host without searching headers
    pub host: &'a str,

    // may not be valid UTF-8
    pub body: &'a [u8],
}

impl<'a> Request<'a> {
    // replace host and url with another host
    pub fn modify_host(&mut self, host: &'a str) {
        // skip "/" in "http://" and find next /
        // we use it as start index of path
        let url = self.path[7..].find("/");
        match url {
            Some(url) => {
                self.path = format!("http://{}/{}", host, &self.path[url + 7..]);
            }
            // None show that url dont have "/" after "http://""
            None => {
                self.path = format!("http://{}/", host);
            }
        }

        // this program use self.host to crate request
        // not self.headers["Host"]
        // buf we still need to change "Host" in headers
        // if not so,web server will send 400 back
        self.host = host;
        for i in 0..self.headers.len() {
            if self.headers[i].key == "Host" {
                self.headers[i].value = host.as_bytes();
            }
        }
    }
    pub fn parse(buf: &'a [u8]) -> Result<Request<'a>, String> {
        // first, find position of body
        let (body_pos, body) = match find(buf, b"\r\n\r\n") {
            Some(pos) => (pos, &buf[pos + 4..]),
            None => (buf.len(), &buf[buf.len()..]),
        };
        // header include all HTTP message header, include first line
        let header = &buf[..body_pos];
        // split it by Line break "\r\n" and return a iterator
        let mut iter = split(header, b"\r\n");
        // find first line and covert it from &[u8] to str
        let first_line = iter.next().ok_or("http massage change line with \\r\\n")?;
        let first_line = std::str::from_utf8(first_line)
            .map_err(|_| "Http message first line contain invalid utf-8")?;
        // split first line by space
        let mut first_line_iter = first_line.split(" ");
        // find method path and version
        let method = first_line_iter
            .next()
            .ok_or("http massage start with method")?;
        let path = first_line_iter
            .next()
            .ok_or("http massage have path")?
            .to_string();
        let version = first_line_iter.next().ok_or("http massage have version")?;
        // find Host in headers
        let mut host = None;
        let headers = iter
            //map header string to struct Header
            .map(|x| {
                let colon_pos = find(x, b":").expect("http header use : split k&v");
                let key = std::str::from_utf8(&x[..colon_pos])
                    .expect("Header key contain invalid utf-8")
                    .trim();
                let colon = std::str::from_utf8(&x[colon_pos..colon_pos + 1])
                    .expect("Header colon contain invalid utf-8");

                // find Host in headers
                if key == "Host" {
                    host = Some(
                        std::str::from_utf8(&x[colon_pos + 1..])
                            .expect("Host contain invalid utf-8")
                            .trim(),
                    )
                }

                // prevent upgrade HTTP to HTTPS
                if key == "Upgrade-Insecure-Requests" {
                    return None;
                }

                Some(Header {
                    key,
                    colon,
                    value: trim(&x[colon_pos + 1..]),
                })
            })
            // filter that None
            .filter(|x| x.is_some())
            // unwrap some in option
            .map(|x| x.expect("impossible"))
            // collect to Vec<Header>
            .collect();
        // return Err when don't find host
        let host = host.ok_or("dont know host")?;
        Ok(Request {
            method,
            path,
            version,
            headers,
            host,
            body,
        })
    }
    // write pain text HTTP request
    pub fn write<T>(&self, f: &mut T) -> std::io::Result<()>
    where
        T: std::io::Write,
    {
        write!(f, "{} {} {}\r\n", self.method, self.path, self.version)?;
        for header in &self.headers {
            write!(f, "{}{} ", header.key, header.colon)?;
            f.write(header.value)?;
            f.write(b"\r\n")?;
        }
        f.write(b"\r\n")?;
        if self.body.len() > 0 {
            f.write(self.body)?;
            f.write(b"\r\n")?;
        }
        Ok(())
    }
}
// display HTTP request message
impl<'a> fmt::Display for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "START HTTP REQUEST\n")?;
        write!(f, "{} {} {}\r\n", self.method, self.path, self.version)?;
        for header in &self.headers {
            write!(f, "{}{} ", header.key, header.colon)?;
            if let Ok(value_str) = std::str::from_utf8(header.value) {
                write!(f, "{}\r\n", value_str)?;
            } else {
                write!(f, "[invalid utf8 key]\r\n")?;
            }
        }
        write!(f, "\r\n")?;
        if self.body.len() > 0 {
            if let Ok(body_str) = std::str::from_utf8(self.body) {
                write!(f, "{}", body_str)?;
            } else {
                write!(f, "[invalid utf8 body]")?;
            }
        }
        write!(f, "END HTTP REQUEST")
    }
}

// some tools for [u8]

// split for [u8]
struct U8SplitIter<'a> {
    buf: &'a [u8],
    pat: &'static [u8],
    pos: usize,
}

impl<'a> Iterator for U8SplitIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        let next_pos = find(&self.buf[self.pos..], self.pat);
        match next_pos {
            Some(next_pos) => {
                let last_pos = self.pos;
                self.pos += next_pos + self.pat.len();
                Some(&self.buf[last_pos..last_pos + next_pos])
            }
            None => {
                if self.pos < self.buf.len() {
                    let last_pos = self.pos;
                    self.pos = self.buf.len();
                    Some(&self.buf[last_pos..])
                } else {
                    None
                }
            }
        }
    }
}

fn find(buf: &[u8], pat: &[u8]) -> Option<usize> {
    assert!(pat.len() > 0);
    for i in 0..buf.len() {
        for j in 0..pat.len() {
            if buf[i + j] != pat[j] {
                break;
            } else {
                if j == pat.len() - 1 {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn split<'a>(buf: &'a [u8], pat: &'static [u8]) -> U8SplitIter<'a> {
    U8SplitIter { buf, pat, pos: 0 }
}

// trim space in [u8]
fn trim<'a>(buf: &'a [u8]) -> &'a [u8] {
    for i in 0..buf.len() {
        if buf[i] != ' ' as u8 {
            return &buf[i..];
        }
    }
    &buf[buf.len()..]
}

// ************TEST*************//

// this is a macro to test Request
macro_rules! req {
    ($name:ident, $buf:expr, |$arg:ident| $body:expr) => {
        #[test]
        fn $name() {
            let req = Request::parse($buf.as_ref()).unwrap();
            fn assert_closure($arg: Request) {
                $body
            }
            assert_closure(req);
        }
    };
}

// copy from rust crate **httparse**
// see https://github.com/seanmonstar/httparse/blob/master/tests/uri.rs

req! {
    urltest_001,
    "GET /bar;par?b HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/bar;par?b");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}

req! {
    urltest_002,
    "GET /x HTTP/1.1\r\nHost: test\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"test");
    }
}

req! {
    urltest_003,
    "GET /x HTTP/1.1\r\nHost: test\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"test");
    }
}

req! {
    urltest_004,
    "GET /foo/foo.com HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/foo.com");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_005,
    "GET /foo/:foo.com HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:foo.com");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_006,
    "GET /foo/foo.com HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/foo.com");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

/*
req! {
    urltest_007,
    "GET  foo.com HTTP/1.1\r\nHost: \r\n\r\n",
    Err(Error::Version),
    |_r| {}
}
*/

req! {
    urltest_008,
    "GET /%20b%20?%20d%20 HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%20b%20?%20d%20");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}

/*
req! {
    urltest_009,
    "GET x x HTTP/1.1\r\nHost: \r\n\r\n",
    Err(Error::Version),
    |_r| {}
}
*/

req! {
    urltest_010,
    "GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}

req! {
    urltest_011,
    "GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}

req! {
    urltest_012,
    "GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}

req! {
    urltest_013,
    "GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}

req! {
    urltest_014,
    "GET /c HTTP/1.1\r\nHost: f\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"f");
    }
}

req! {
    urltest_015,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_016,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_017,
    "GET /foo/:foo.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:foo.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_018,
    "GET /foo/:foo.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:foo.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_019,
    "GET /foo/: HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_020,
    "GET /foo/:a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_021,
    "GET /foo/:/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_022,
    "GET /foo/:/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_023,
    "GET /foo/: HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_024,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_025,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_026,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_027,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_028,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_029,
    "GET /foo/:23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:23");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_030,
    "GET /:23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/:23");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_031,
    "GET /foo/:: HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/::");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_032,
    "GET /foo/::23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/::23");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_033,
    "GET /d HTTP/1.1\r\nHost: c\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/d");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"c");
    }
}

req! {
    urltest_034,
    "GET /foo/:@c:29 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/:@c:29");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_035,
    "GET //@ HTTP/1.1\r\nHost: foo.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "//@");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo.com");
    }
}

req! {
    urltest_036,
    "GET /b:c/d@foo.com/ HTTP/1.1\r\nHost: a\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/b:c/d@foo.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"a");
    }
}

req! {
    urltest_037,
    "GET /bar.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/bar.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_038,
    "GET /////// HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "///////");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_039,
    "GET ///////bar.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "///////bar.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_040,
    "GET //:///// HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "//://///");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_041,
    "GET /foo HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_042,
    "GET /bar HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}

req! {
    urltest_043,
    "GET /path;a??e HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/path;a??e");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}

req! {
    urltest_044,
    "GET /abcd?efgh?ijkl HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/abcd?efgh?ijkl");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}

req! {
    urltest_045,
    "GET /abcd HTTP/1.1\r\nHost: foo\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/abcd");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo");
    }
}

req! {
    urltest_046,
    "GET /foo/[61:24:74]:98 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/[61:24:74]:98");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_047,
    "GET /foo/[61:27]/:foo HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/[61:27]/:foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_048,
    "GET /example.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_049,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_050,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_051,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_052,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_053,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_054,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_055,
    "GET /foo/example.com/ HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_056,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_057,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_058,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_059,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_060,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_061,
    "GET /a/b/c HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/a/b/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_062,
    "GET /a/%20/c HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/a/%20/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_063,
    "GET /a%2fc HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/a%2fc");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_064,
    "GET /a/%2f/c HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/a/%2f/c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_065,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_066,
    "GET text/html,test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "text/html,test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_067,
    "GET 1234567890 HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "1234567890");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_068,
    "GET /c:/foo/bar.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c:/foo/bar.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_069,
    "GET /c:////foo/bar.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c:////foo/bar.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_070,
    "GET /C:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_071,
    "GET /C:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_072,
    "GET /C:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_073,
    "GET /file HTTP/1.1\r\nHost: server\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/file");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"server");
    }
}

req! {
    urltest_074,
    "GET /file HTTP/1.1\r\nHost: server\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/file");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"server");
    }
}

req! {
    urltest_075,
    "GET /file HTTP/1.1\r\nHost: server\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/file");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"server");
    }
}

req! {
    urltest_076,
    "GET /foo/bar.txt HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_077,
    "GET /home/me HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/home/me");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_078,
    "GET /test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_079,
    "GET /test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_080,
    "GET /tmp/mock/test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/tmp/mock/test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_081,
    "GET /tmp/mock/test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/tmp/mock/test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_082,
    "GET /foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_083,
    "GET /.foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/.foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_084,
    "GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_085,
    "GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_086,
    "GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_087,
    "GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_088,
    "GET /foo/..bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/..bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_089,
    "GET /foo/ton HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/ton");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_090,
    "GET /a HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_091,
    "GET /ton HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/ton");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_092,
    "GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_093,
    "GET /foo/%2e%2 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/%2e%2");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_094,
    "GET /%2e.bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%2e.bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_095,
    "GET // HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "//");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_096,
    "GET /foo/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_097,
    "GET /foo/bar/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_098,
    "GET /foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_099,
    "GET /%20foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%20foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_100,
    "GET /foo% HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_101,
    "GET /foo%2 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%2");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_102,
    "GET /foo%2zbar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%2zbar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_103,
    "GET /foo%2%C3%82%C2%A9zbar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%2%C3%82%C2%A9zbar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_104,
    "GET /foo%41%7a HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%41%7a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_105,
    "GET /foo%C2%91%91 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%C2%91%91");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_106,
    "GET /foo%00%51 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%00%51");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_107,
    "GET /(%28:%3A%29) HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/(%28:%3A%29)");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_108,
    "GET /%3A%3a%3C%3c HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%3A%3a%3C%3c");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_109,
    "GET /foobar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foobar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_110,
    "GET //foo//bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "//foo//bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_111,
    "GET /%7Ffp3%3Eju%3Dduvgw%3Dd HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%7Ffp3%3Eju%3Dduvgw%3Dd");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_112,
    "GET /@asdf%40 HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/@asdf%40");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_113,
    "GET /%E4%BD%A0%E5%A5%BD%E4%BD%A0%E5%A5%BD HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%E4%BD%A0%E5%A5%BD%E4%BD%A0%E5%A5%BD");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_114,
    "GET /%E2%80%A5/foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%E2%80%A5/foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_115,
    "GET /%EF%BB%BF/foo HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%EF%BB%BF/foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_116,
    "GET /%E2%80%AE/foo/%E2%80%AD/bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%E2%80%AE/foo/%E2%80%AD/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_117,
    "GET /foo?bar=baz HTTP/1.1\r\nHost: www.google.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo?bar=baz");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.google.com");
    }
}

req! {
    urltest_118,
    "GET /foo?bar=baz HTTP/1.1\r\nHost: www.google.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo?bar=baz");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.google.com");
    }
}

req! {
    urltest_119,
    "GET test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_120,
    "GET /foo%2Ehtml HTTP/1.1\r\nHost: www\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo%2Ehtml");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www");
    }
}

req! {
    urltest_121,
    "GET /foo/html HTTP/1.1\r\nHost: www\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www");
    }
}

req! {
    urltest_122,
    "GET /foo HTTP/1.1\r\nHost: www.google.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.google.com");
    }
}

req! {
    urltest_123,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_124,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_125,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_126,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_127,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_128,
    "GET /example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_129,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_130,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_131,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_132,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_133,
    "GET example.com/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "example.com/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_134,
    "GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_135,
    "GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_136,
    "GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_137,
    "GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_138,
    "GET /aaa/test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/aaa/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_139,
    "GET /test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_140,
    "GET /%E4%B8%AD/test.txt HTTP/1.1\r\nHost: www.example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%E4%B8%AD/test.txt");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"www.example.com");
    }
}

req! {
    urltest_141,
    "GET /... HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/...");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_142,
    "GET /a HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_143,
    "GET /%EF%BF%BD?%EF%BF%BD HTTP/1.1\r\nHost: x\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%EF%BF%BD?%EF%BF%BD");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"x");
    }
}

req! {
    urltest_144,
    "GET /bar HTTP/1.1\r\nHost: example.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.com");
    }
}

req! {
    urltest_145,
    "GET test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_146,
    "GET x@x.com HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "x@x.com");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_147,
    "GET, HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, ",");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_148,
    "GET blank HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "blank");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_149,
    "GET test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "test?test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_150,
    "GET /%60%7B%7D?`{} HTTP/1.1\r\nHost: h\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/%60%7B%7D?`{}");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"h");
    }

}

req! {
    urltest_151,
    "GET /?%27 HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/?%27");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}

req! {
    urltest_152,
    "GET /?' HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/?'");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}

req! {
    urltest_153,
    "GET /some/path HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/some/path");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_154,
    "GET /smth HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/smth");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_155,
    "GET /some/path HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/some/path");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_156,
    "GET /pa/i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_157,
    "GET /i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}

req! {
    urltest_158,
    "GET /pa/i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_159,
    "GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_160,
    "GET /i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}

req! {
    urltest_161,
    "GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_162,
    "GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_163,
    "GET /i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}

req! {
    urltest_164,
    "GET /i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_165,
    "GET /pa/pa?i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa/pa?i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_166,
    "GET /pa?i HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa?i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}

req! {
    urltest_167,
    "GET /pa/pa?i HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa/pa?i");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_168,
    "GET sd HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "sd");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_169,
    "GET sd/sd HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "sd/sd");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_170,
    "GET /pa/pa HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa/pa");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_171,
    "GET /pa HTTP/1.1\r\nHost: ho\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"ho");
    }
}

req! {
    urltest_172,
    "GET /pa/pa HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pa/pa");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_173,
    "GET /x HTTP/1.1\r\nHost: %C3%B1\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"%C3%B1");
    }
}

/*
req! {
    urltest_174,
    "GET \\.\\./ HTTP/1.1\r\nHost: \r\n\r\n",
    Err(Error::Token),
    |_r| {}
}
*/

req! {
    urltest_175,
    "GET :a@example.net HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, ":a@example.net");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_176,
    "GET %NBD HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "%NBD");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_177,
    "GET %1G HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "%1G");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_178,
    "GET /relative_import.html HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/relative_import.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"127.0.0.1");
    }
}

req! {
    urltest_179,
    "GET /?foo=%7B%22abc%22 HTTP/1.1\r\nHost: facebook.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/?foo=%7B%22abc%22");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"facebook.com");
    }
}

req! {
    urltest_180,
    "GET /jqueryui@1.2.3 HTTP/1.1\r\nHost: localhost\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/jqueryui@1.2.3");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"localhost");
    }
}

req! {
    urltest_181,
    "GET /path?query HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/path?query");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}

req! {
    urltest_182,
    "GET /foo/bar?a=b&c=d HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar?a=b&c=d");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_183,
    "GET /foo/bar??a=b&c=d HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar??a=b&c=d");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_184,
    "GET /foo/bar HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_185,
    "GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/baz?qux");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}

req! {
    urltest_186,
    "GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/baz?qux");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}

req! {
    urltest_187,
    "GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/baz?qux");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}

req! {
    urltest_188,
    "GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/baz?qux");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}

req! {
    urltest_189,
    "GET /baz?qux HTTP/1.1\r\nHost: foo.bar\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/baz?qux");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foo.bar");
    }
}

req! {
    urltest_190,
    "GET /C%3A/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C%3A/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_191,
    "GET /C%7C/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C%7C/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_192,
    "GET /C:/Users/Domenic/Dropbox/GitHub/tmpvar/jsdom/test/level2/html/files/pix/submit.gif HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/Users/Domenic/Dropbox/GitHub/tmpvar/jsdom/test/level2/html/files/pix/submit.gif");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_193,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_194,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_195,
    "GET /d: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/d:");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_196,
    "GET /d:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/d:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_197,
    "GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_198,
    "GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_199,
    "GET /test?x HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_200,
    "GET /test?x HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_201,
    "GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_202,
    "GET /test?test HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_203,
    "GET /?fox HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/?fox");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_204,
    "GET /localhost//cat HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/localhost//cat");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_205,
    "GET /localhost//cat HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/localhost//cat");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_206,
    "GET /mouse HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/mouse");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_207,
    "GET /pig HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pig");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_208,
    "GET /pig HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pig");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_209,
    "GET /pig HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/pig");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_210,
    "GET /localhost//pig HTTP/1.1\r\nHost: lion\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/localhost//pig");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"lion");
    }
}

req! {
    urltest_211,
    "GET /rooibos HTTP/1.1\r\nHost: tea\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/rooibos");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"tea");
    }
}

req! {
    urltest_212,
    "GET /?chai HTTP/1.1\r\nHost: tea\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/?chai");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"tea");
    }
}

req! {
    urltest_213,
    "GET /C: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_214,
    "GET /C: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_215,
    "GET /C: HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_216,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_217,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_218,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_219,
    "GET /dir/C HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/dir/C");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}

req! {
    urltest_220,
    "GET /dir/C|a HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/dir/C|a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}

req! {
    urltest_221,
    "GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_222,
    "GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_223,
    "GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_224,
    "GET /c:/foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/c:/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_225,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_226,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_227,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_228,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_229,
    "GET /C:/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/C:/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_230,
    "GET /?q=v HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/?q=v");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_231,
    "GET ?x HTTP/1.1\r\nHost: %C3%B1\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "?x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"%C3%B1");
    }
}

req! {
    urltest_232,
    "GET ?x HTTP/1.1\r\nHost: %C3%B1\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "?x");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"%C3%B1");
    }
}

req! {
    urltest_233,
    "GET // HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "//");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_234,
    "GET //x/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "//x/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_235,
    "GET /someconfig;mode=netascii HTTP/1.1\r\nHost: foobar.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/someconfig;mode=netascii");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"foobar.com");
    }
}

req! {
    urltest_236,
    "GET /Index.ut2 HTTP/1.1\r\nHost: 10.10.10.10\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/Index.ut2");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"10.10.10.10");
    }
}

req! {
    urltest_237,
    "GET /0?baz=bam&qux=baz HTTP/1.1\r\nHost: somehost\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/0?baz=bam&qux=baz");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"somehost");
    }
}

req! {
    urltest_238,
    "GET /sup HTTP/1.1\r\nHost: host\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/sup");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"host");
    }
}

req! {
    urltest_239,
    "GET /foo/bar.git HTTP/1.1\r\nHost: github.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar.git");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"github.com");
    }
}

req! {
    urltest_240,
    "GET /channel?passwd HTTP/1.1\r\nHost: myserver.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/channel?passwd");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"myserver.com");
    }
}

req! {
    urltest_241,
    "GET /foo.bar.org?type=TXT HTTP/1.1\r\nHost: fw.example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo.bar.org?type=TXT");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"fw.example.org");
    }
}

req! {
    urltest_242,
    "GET /ou=People,o=JNDITutorial HTTP/1.1\r\nHost: localhost\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/ou=People,o=JNDITutorial");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"localhost");
    }
}

req! {
    urltest_243,
    "GET /foo/bar HTTP/1.1\r\nHost: github.com\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"github.com");
    }
}

req! {
    urltest_244,
    "GET ietf:rfc:2648 HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "ietf:rfc:2648");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_245,
    "GET joe@example.org,2001:foo/bar HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "joe@example.org,2001:foo/bar");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_246,
    "GET /path HTTP/1.1\r\nHost: H%4fSt\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/path");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"H%4fSt");
    }
}

req! {
    urltest_247,
    "GET https://example.com:443/ HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "https://example.com:443/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_248,
    "GET d3958f5c-0777-0845-9dcf-2cb28783acaf HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "d3958f5c-0777-0845-9dcf-2cb28783acaf");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_249,
    "GET /test?%22 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?%22");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_250,
    "GET /test HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_251,
    "GET /test?%3C HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?%3C");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_252,
    "GET /test?%3E HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?%3E");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_253,
    "GET /test?%E2%8C%A3 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?%E2%8C%A3");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_254,
    "GET /test?%23%23 HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?%23%23");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_255,
    "GET /test?%GH HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?%GH");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_256,
    "GET /test?a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_257,
    "GET /test?a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}

req! {
    urltest_258,
    "GET /test-a-colon-slash.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test-a-colon-slash.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_259,
    "GET /test-a-colon-slash-slash.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test-a-colon-slash-slash.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_260,
    "GET /test-a-colon-slash-b.html HTTP/1.1\r\nHost: \r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test-a-colon-slash-b.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"");
    }
}

req! {
    urltest_261,
    "GET /test-a-colon-slash-slash-b.html HTTP/1.1\r\nHost: b\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test-a-colon-slash-slash-b.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"b");
    }
}

req! {
    urltest_262,
    "GET /test?a HTTP/1.1\r\nHost: example.org\r\n\r\n",
    |req| {
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/test?a");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Host");
        assert_eq!(req.headers[0].value, b"example.org");
    }
}
