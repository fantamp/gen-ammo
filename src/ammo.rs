extern crate logut;
use logut::LogRecord;
use std::io::prelude::*;
use std;
use std::io::Cursor;

/// View to ammo data with essential fields extracted
pub struct BulletData<'a> {
    pub resource: &'a [u8],
    pub host: &'a [u8],
    pub place: &'a [u8],
    pub wizards: &'a [u8],
}

/// If we need to own data
pub struct StoredBullet {
    pub resource: Vec<u8>,
    pub host: Vec<u8>,
    pub place: Vec<u8>,
    pub wizards: Vec<u8>,
}

impl StoredBullet {
    pub fn from_data(data: &BulletData) -> StoredBullet {
        StoredBullet {
            resource: data.resource.to_vec(),
            host: data.host.to_vec(),
            place: data.place.to_vec(),
            wizards: data.wizards.to_vec(),
        }
    }

    pub fn get_data(&self) -> BulletData {
        BulletData {
            resource: &self.resource,
            host: &self.host,
            place: &self.place,
            wizards: &self.wizards,
        }
    }
}

pub fn write_bullet<W: Write>(bullet: &BulletData, buff: &mut Cursor<Vec<u8>>, to: &mut W) -> std::io::Result<()> {
    buff.write(b"GET /")?;
    buff.write(bullet.resource)?;
    buff.write(
        b" HTTP/1.0\r\n\
        User-Agent: tank\r\n\
        Connection: close\r\n\
        \r\n")?;
    write!(to, "{} ", buff.position())?;
    // write tags
    {
        if bullet.place.len() > 0 {
            to.write(bullet.place)?;
        }
        // TODO: shorten wizards names using re.sub(r'([aeiouy])', '', w, flags=re.IGNORECASE)
        // or map well-known wizard names to some predefined short names
        for wzrd in bullet.wizards.split(|b| *b == b',').filter(|x| x.len() > 0) {
            to.write(b"|")?;
            to.write(wzrd)?;
        }
    }
    to.write(b"\r\n")?;
    buff.set_position(0);
    std::io::copy(buff, to)?;
    to.write(b"\r\n")?;
    Ok(())
}

pub fn make_bullet_data_from_log_record(rec: LogRecord) -> BulletData {
    let (host, _, resource) = logut::get_host_port_resource_from_url(rec.url);
    let place = logut::get_cgi_param_value_naive(resource, b"place").unwrap_or(b"");
    BulletData {
        resource: resource,
        host: host,
        place: place,
        wizards: rec.wizards
    }
}


#[cfg(test)]
mod tests {
    use logut::*;
    use super::BulletData;

    #[test]
    fn test_make_bullet_data_from_log_record() {
        {
            let rec = LogRecord {
                url: b"http://aaaa.bazar.bububu.net:12022/search?base=default.bazar-exp.fro01ht.bububu.ru&ip=&ip-xxds=1203&bububuuid=449823&puid=3975&currency=RUR&fuid=&place=prime&history_itemsts=",
                wizards: b"wiz1,wiz2,wiz3",
            };
            let data = super::make_bullet_data_from_log_record(rec);
            assert_eq!(data.resource, b"search?base=default.bazar-exp.fro01ht.bububu.ru&ip=&ip-xxds=1203&bububuuid=449823&puid=3975&currency=RUR&fuid=&place=prime&history_itemsts=".as_ref());
            assert_eq!(data.place, b"prime".as_ref());
            assert_eq!(data.host, b"aaaa.bazar.bububu.net".as_ref());
            assert_eq!(data.wizards, b"wiz1,wiz2,wiz3".as_ref());
        }
        {
            let rec = LogRecord {
                url: b"",
                wizards: b"",
            };
            let data = super::make_bullet_data_from_log_record(rec);
            assert_eq!(data.resource, b"".as_ref());
            assert_eq!(data.place, b"".as_ref());
            assert_eq!(data.host, b"".as_ref());
            assert_eq!(data.wizards, b"".as_ref());
        }
    }

    #[test]
    fn test_write_bullet() {
        use std::io::Cursor;
        let b = BulletData {
            host: b"localhost",
            resource: b"/search?place=dubai",
            place: b"dubai",
            wizards: b"",
        };
        let mut buff = Cursor::new(vec![0; 15]);
        let mut dest = Cursor::new(vec![0; 15]);
        super::write_bullet(&b, &mut buff, &mut dest).unwrap();

        //TODO: check content
    }
}
