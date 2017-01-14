use std::io::{self, Read, BufRead, BufReader, Cursor};
use std::fs::File;
use std::path::PathBuf;
extern crate flate2;
use flate2::read::GzDecoder;

pub trait ReadByLine {
    fn process_lines(&mut self, feed_to: &mut FnMut(&[u8])) -> io::Result<()>;
}

/// Detects file encoding and calls feed_to for each line
fn process_lines(raw: &mut BufRead, feed_to: &mut FnMut(&[u8])) -> io::Result<()>
{
    let prefetched = {
        let mut v: Vec<u8> = vec![0; 128];
        let count = raw.read(&mut v)?;
        v.resize(count, 0);
        v
    };

    let mut reader: Box<BufRead> = match GzDecoder::new(Cursor::new(&prefetched)) {
        Err(_) => { Box::new(Cursor::new(&prefetched).chain(raw)) },
        Ok(_) => { Box::new(BufReader::new(GzDecoder::new(Cursor::new(&prefetched).chain(raw))?)) },
    };

    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line)? > 0 {
        {
            while *line.last().unwrap_or(&b'\0') == b'\n' {
                line.pop();
            }
            feed_to(&line);
        }
        line.clear();
    }
    Ok(())
}


pub struct Chained {
    pub sources: Vec<Box<ReadByLine>>,
}

impl ReadByLine for Chained {
    fn process_lines(&mut self, feed_to: &mut FnMut(&[u8])) -> io::Result<()>
    {
        for i in 0..self.sources.len() {
            self.sources[i].process_lines(&mut |line: &[u8]| { feed_to(line) })?;
        }
        // for path in &self.paths {
        //     let file = Box::new(File::open(path)?);
        //     let mut reader = BufReader::new(file);
        //     process_lines(&mut reader, feed_to)?;
        // }
        Ok(())
    }
}

pub struct GenericReader {
    pub reader: Box<BufRead>,
    // pub reader: Box<Read>,
}

impl ReadByLine for GenericReader {
    fn process_lines(&mut self, feed_to: &mut FnMut(&[u8])) -> io::Result<()>
    {
        // let mut buf = Box::new(BufReader::new(self.reader));
        process_lines(&mut self.reader, feed_to)
    }
}

pub struct FileLinesReader {
    pub filename: PathBuf,
}

impl ReadByLine for FileLinesReader {
    fn process_lines(&mut self, feed_to: &mut FnMut(&[u8])) -> io::Result<()>
    {
        let file = Box::new(File::open(&self.filename)?);
        let buf = Box::new(BufReader::new(file));
        let mut reader = GenericReader {reader: buf };
        reader.process_lines(feed_to)
    }

}

pub struct FromStdin;

impl ReadByLine for FromStdin {
    fn process_lines(&mut self, feed_to: &mut FnMut(&[u8])) -> io::Result<()>
    {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        process_lines(&mut handle, feed_to)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;
    use std::io::Write;

    #[test]
    fn proc_lines() {
        let mut buf = Cursor::new(vec![]);
        write!(&mut buf, "line one\n").unwrap();
        write!(&mut buf, "line two\n").unwrap();
        write!(&mut buf, "line three\n").unwrap();
        buf.set_position(0);
        let mut res: Vec<Vec<u8>> = vec![];
        super::process_lines(&mut buf, &mut |line: &[u8]| res.push(line.to_vec())).unwrap();
        assert_eq!(res[0], b"line one");
        assert_eq!(res[1], b"line two");
        assert_eq!(res[2], b"line three");
    }

    #[test]
    fn proc_lines_gz() {
        let gz: Vec<u8> = vec![0x1f, 0x8b, 0x8, 0x8, 0xa3, 0x9b, 0x6e, 0x58, 0x0, 0x3, 0x31, 0x2e,
            0x74, 0x78, 0x74, 0x0, 0xcb, 0xc9, 0xcc, 0x4b, 0x55, 0xc8, 0xcf, 0x4b, 0xe5, 0xca, 0x1,
            0x31, 0x4a, 0xca, 0xf3, 0xa1, 0x8c, 0x8c, 0xa2, 0xd4, 0x54, 0x2e, 0x0, 0x2e, 0x18, 0x8f,
            0x57, 0x1d, 0x0, 0x0, 0x0];
        let mut buf = Cursor::new(gz);
        let mut res: Vec<Vec<u8>> = vec![];
        super::process_lines(&mut buf, &mut |line: &[u8]| res.push(line.to_vec())).unwrap();
        assert_eq!(res[0], b"line one");
        assert_eq!(res[1], b"line two");
        assert_eq!(res[2], b"line three");
    }
}
