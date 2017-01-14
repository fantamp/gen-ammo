extern crate flate2;
extern crate twoway;

pub mod read;

/// View to log line with essential fields extracted
pub struct LogRecord<'a> {
    pub url: &'a [u8],
    pub wizards: &'a [u8],
}

/// Make LogRecord from line containing only url
///
/// # Examples:
///
/// ```
/// use logut::make_record_from_plain_line;
/// let rec = make_record_from_plain_line(b"http://example.com");
/// assert_eq!(rec.url, b"http://example.com");
/// assert_eq!(rec.wizards, b"");
/// ```
pub fn make_record_from_plain_line(line: &[u8]) -> LogRecord {
    LogRecord { url: line, wizards: b"" }
}

/// Make LogRecord from tab-separated log line
///
/// # Examples:
///
/// ```
/// use logut::parse_tab_separated_log_line;
/// let rec = parse_tab_separated_log_line(b"[date]\thttp://example.com\t\t\t\t\t\t\t\t\t\t\tbebebe,zz\t\t");
/// assert_eq!(rec.url, b"http://example.com");
/// assert_eq!(rec.wizards, b"bebebe,zz");
/// ```
pub fn parse_tab_separated_log_line(line: &[u8]) -> LogRecord {
    let mut url = None;
    let mut wizards = None;
    for (i, value) in line.split(|b| *b == b'\t').enumerate() {
        match i {
            1 => url = Some(value),
            12 => wizards = Some(value),
            13 => break,
            _ => {},
        }
    }
    LogRecord { url: url.unwrap_or(b""), wizards: wizards.unwrap_or(b"") }
}

/// Make LogRecord from tskv-formated log line
///
/// # Examples
/// ```
/// use logut::parse_tskv_log_line;
/// let rec = parse_tskv_log_line(b"tskv\turl=http://example.com\twizards=bebebe,zz");
/// assert_eq!(rec.url, b"http://example.com");
/// assert_eq!(rec.wizards, b"bebebe,zz");
/// ```
pub fn parse_tskv_log_line(line: &[u8]) -> LogRecord {
    let mut url: Option<&[u8]> = None;
    let mut wizards: Option<&[u8]> = None;
    for item in line.split(|b| *b == b'\t') {
        let (key, value) = {
            let mut iter = item.splitn(2, |b| *b == b'=');
            (iter.next().unwrap_or(b""), iter.next().unwrap_or(b""))
        };
        match key {
            b"url" => url = Some(value),
            b"wizards" => wizards = Some(value),
            _ => {},
        }
    }
    LogRecord {
        url: url.unwrap_or(b""),
        wizards: wizards.unwrap_or(b""),
    }
}

/// Make LogRecord from log line of variety of formats
///
/// # Examples:
///
/// ```
/// use logut::parse_log_line;
/// let rec = parse_log_line(b"tskv\turl=http://example.com\twizards=bebebe,zz");
/// assert_eq!(rec.url, b"http://example.com");
/// assert_eq!(rec.wizards, b"bebebe,zz");
///
/// let rec = parse_log_line(b"[date]\thttp://example.com\t\t\t\t\t\t\t\t\t\t\tbebebe,zz\t\t");
/// assert_eq!(rec.url, b"http://example.com");
/// assert_eq!(rec.wizards, b"bebebe,zz");
///
/// let rec = parse_log_line(b"http://example.com");
/// assert_eq!(rec.url, b"http://example.com");
/// assert_eq!(rec.wizards, b"");
/// ```
pub fn parse_log_line(line: &[u8]) -> LogRecord {
    if !line.starts_with(b"tskv") && !line.starts_with(b"[") {
        make_record_from_plain_line(line)
    } else {
        if line.split(|b| *b == b'\t').next().unwrap_or(b"") == b"tskv" {
            parse_tskv_log_line(line)
        } else {
            parse_tab_separated_log_line(line)
        }
    }
}

/// Returns value of CGI param using naive but quite fast approach
///
/// # Examples:
///
/// ```
/// use logut::get_cgi_param_value_naive;
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?place=moscow", b"place").unwrap(), b"moscow");
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?pp=18&place=moscow", b"place").unwrap(), b"moscow");
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?pp=18&place=dubai&key=value", b"place").unwrap(), b"dubai");
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?pp=18&place=&text=lalala", b"place").unwrap(), b"");
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?pp=18&place=", b"place").unwrap(), b"");
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?pp=18&xxxplace=", b"place"), None);
/// assert_eq!(get_cgi_param_value_naive(b"http://example.com?pp=18&noparam=1", b"place"), None);
/// assert_eq!(get_cgi_param_value_naive(b"search?pp=18&&text=place=jopa&place=dubai&sdfd", b"place").unwrap(), b"dubai");
/// assert_eq!(get_cgi_param_value_naive(b"", b"place"), None);
/// assert_eq!(get_cgi_param_value_naive(b"", b""), None);
/// ```
pub fn get_cgi_param_value_naive<'a>(url: &'a [u8], param: &[u8]) -> Option<&'a [u8]> {
    for part in url.split(|b| *b==b'?' || *b==b'&') {
        if part.starts_with(param) && part.get(param.len()) == Some(&b'=') {
            return Some(&part[param.len()+1..]);
        }
    }
    None
}

/// Extracts host, url and resource parts from given URL
///
/// # Examples:
///
/// ```
/// use logut::get_host_port_resource_from_url;
/// assert_eq!(get_host_port_resource_from_url(b""), (b"".as_ref(), b"".as_ref(), b"".as_ref()));
/// assert_eq!(get_host_port_resource_from_url(b"http://"), (b"".as_ref(), b"".as_ref(), b"".as_ref()));
/// assert_eq!(get_host_port_resource_from_url(b"http://example.com"), (b"example.com".as_ref(), b"".as_ref(), b"".as_ref()));
/// assert_eq!(get_host_port_resource_from_url(b"http://:80"), (b"".as_ref(), b"80".as_ref(), b"".as_ref()));
/// assert_eq!(get_host_port_resource_from_url(b"http://example.com:80"), (b"example.com".as_ref(), b"80".as_ref(), b"".as_ref()));
/// assert_eq!(get_host_port_resource_from_url(b"http://example.com:80/"), (b"example.com".as_ref(), b"80".as_ref(), b"".as_ref()));
/// assert_eq!(get_host_port_resource_from_url(b"example.com"), (b"example.com".as_ref(), b"".as_ref(), b"".as_ref()));
/// ```
pub fn get_host_port_resource_from_url(url: &[u8]) -> (&[u8], &[u8], &[u8]) {
    let url = match url.starts_with(b"http://") {
        true => &url[7..],
        false => url,
    };

    let (host_port, resource) = match twoway::find_bytes(url, b"/") {
        Some(pos) => (&url[..pos], &url[pos+1 ..]),
        None => (url, b"".as_ref()),
    };

    let (host, port) = match twoway::find_bytes(host_port, b":") {
        Some(pos) => (&host_port[..pos], &host_port[pos+1 ..]),
        None => (host_port, b"".as_ref()),
    };

    (host, port, resource)
}


#[cfg(test)]
mod tests {
    use twoway;

    #[test]
    fn test_parse_tab_separated_log_line() {
        {
            let rec = super::parse_tab_separated_log_line(b"");
            assert_eq!(rec.url, b"");
            assert_eq!(rec.wizards, b"");
        }
        {
            let rec = super::parse_tab_separated_log_line(b"zzz");
            assert_eq!(rec.url, b"");
            assert_eq!(rec.wizards, b"");
        }
        {
            let rec = super::parse_tab_separated_log_line(b"zzz\tbebebe");
            assert_eq!(rec.url, b"bebebe");
            assert_eq!(rec.wizards, b"");
        }
        {
            let rec = super::parse_tab_separated_log_line(b"\tbebebe\t");
            assert_eq!(rec.url, b"bebebe");
            assert_eq!(rec.wizards, b"");
        }
        {
            let line = b"[Tue Dec 13 06:28:45 2016]\thttp://example.com:17051/search?base=default.exp.pepe01ht.example.com\tundefined\t12\t\t517762881\t::1\t12\t0\t0\t0\t-1\twiz1,wiz2\tf738c8xxxxxxx1ac27ex0b6fbe9\t\t\t\t6\t5\t0\t1\t4\t3\tc2c4937cxxxxxxxxd82713088e401\t\t\t0\t14815997xxxxxxxxxxxx6fbe9/1\t9999999725766\t0\tNONE";
            let rec = super::parse_tab_separated_log_line(line);
            let needle: &[u8] = b"http://example.com:17051/search?base=default.exp.pepe01ht.example.com";
            assert_eq!(rec.url, needle);
            assert_eq!(rec.wizards, b"wiz1,wiz2");
        }
    }


    #[test]
    fn test_parse_tskv_log_line() {
        let rec = super::parse_tskv_log_line(b"tskv\turl=http://bfg9000:5000/zomb/\twizards=1,2,3,4,5\tx=value");
        assert_eq!(rec.url, b"http://bfg9000:5000/zomb/");
        assert_eq!(rec.wizards, b"1,2,3,4,5");
    }

    #[test]
    fn test_parse_log_line() {
        {
            let rec = super::parse_log_line(b"tskv\turl=http://bfg9000:5000/zomb/\twizards=1,2,3,4,5\tx=value");
            assert_eq!(rec.url, b"http://bfg9000:5000/zomb/");
            assert_eq!(rec.wizards, b"1,2,3,4,5");
        }
        {
            let rec = super::parse_log_line(b"tskv");
            assert_eq!(rec.url, b"");
            assert_eq!(rec.wizards, b"");
        }
        {
            let rec = super::parse_log_line(b"tskv\t");
            assert_eq!(rec.url, b"");
            assert_eq!(rec.wizards, b"");
        }
        {
            let rec = super::parse_log_line(b"");
            assert_eq!(rec.url, b"");
            assert_eq!(rec.wizards, b"");
        }
        {
            let rec = super::parse_tab_separated_log_line(b"zzz\tbebebe\t\t");
            assert_eq!(rec.url, b"bebebe");
            assert_eq!(rec.wizards, b"");
        }
    }

    #[test]
    fn twoway_simple() {
        let text = b"Hello, world!";
        let pattern = b"llo, ";
        let pos = twoway::find_bytes(text, pattern);
        assert_eq!(Some(2), pos);
    }
}
