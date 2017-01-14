use rand;
use rand::Rng;
use error::ProcError;
use ammo::*;
use std::io;
use std::io::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::process::{Command, Stdio, Child};
use std::path::Path;

pub trait AmmoProcessor {
    fn process(&mut self, bullet: &BulletData) -> Result<(), ProcError>;
    fn finish(&mut self) -> Result<(), ProcError> {
        Ok(())
    }
}

pub struct ReserviorSampling {
    selected: Vec<StoredBullet>,
    target_set_size: usize,
    index: usize,
    rng: Box<rand::Rng>,
    subprocessor: Box<AmmoProcessor>,
}

 impl ReserviorSampling {
    pub fn new(set_size: usize, subprocessor: Box<AmmoProcessor>) -> ReserviorSampling {
        ReserviorSampling {
            target_set_size: set_size,
            selected: Vec::with_capacity(set_size),
            index: 0,
            rng: Box::new(rand::thread_rng()),
            subprocessor: subprocessor
        }
    }
}

impl AmmoProcessor for ReserviorSampling {
    fn process(&mut self, bullet: &BulletData) -> Result<(), ProcError> {
        if self.index < self.target_set_size {
            self.selected.push(StoredBullet::from_data(bullet));
        } else {
            let r = self.rng.gen_range(0, self.index);
            if r < self.target_set_size {
                self.selected[r] = StoredBullet::from_data(bullet);
            }
        }
        self.index += 1;
        Ok(())
    }
    fn finish(&mut self) -> Result<(), ProcError> {
        if self.selected.len() < self.target_set_size {
            Err(ProcError::Logic(format!("Not enough input lines: have seen {} but at least {} were expected", self.index, self.target_set_size)))
        } else {
            for bullet in &self.selected {
                try!(self.subprocessor.process(&bullet.get_data()));
            }
            self.subprocessor.finish()
        }
    }
}

pub struct MethodS {
    input_lines_count: usize,
    target_set_size: usize,
    already_processed: usize,
    already_selected: usize,
    rng: Box<rand::Rng>,
    subprocessor: Box<AmmoProcessor>,
}

impl MethodS {
    pub fn new(input_lines_count: usize, target_set_size: usize, subprocessor: Box<AmmoProcessor>) -> Box<AmmoProcessor> {
        if input_lines_count < target_set_size {
            panic!("Not enough input lines: have {} but at least {} is needed", input_lines_count, target_set_size)
        }
        let p = MethodS {
            input_lines_count: input_lines_count,
            target_set_size: target_set_size,
            already_processed: 0,
            already_selected: 0,
            rng: Box::new(rand::thread_rng()),
            subprocessor: subprocessor
        };
        Box::new(p)
    }
}

impl AmmoProcessor for MethodS {
    fn process(&mut self, bullet: &BulletData) -> Result<(), ProcError> {
        let need = self.target_set_size - self.already_selected;
        let not_seen = self.input_lines_count - self.already_processed;
        let rnd = 1 + self.rng.gen_range(0, not_seen);
        if need >= rnd {
            self.already_selected += 1;
            try!(self.subprocessor.process(bullet));
        }
        self.already_processed += 1;
        Ok(())
    }
    fn finish(&mut self) -> Result<(), ProcError> {
        self.subprocessor.finish()
    }
}


// TODO: use std::iter::Cycle; iterator instead! But it isn't so easy!
pub struct RoundRobin {
    subprocessors: Vec<Box<AmmoProcessor>>,
    current: usize
}

impl RoundRobin {
    pub fn new(subprocessors: Vec<Box<AmmoProcessor>>) -> RoundRobin {
        RoundRobin {
            subprocessors: subprocessors,
            current: 0,
        }
    }
}

impl AmmoProcessor for RoundRobin {
    fn process(&mut self, bullet: &BulletData) -> Result<(), ProcError> {
        try!(self.subprocessors[self.current].process(bullet));
        self.current += 1;
        if self.current >= self.subprocessors.len() {
            self.current = 0;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), ProcError> {
        for consumer in self.subprocessors.iter_mut() {
            try!(consumer.finish());
        }
        Ok(())
    }
}

pub struct WriteAmmo {
    buff: io::Cursor<Vec<u8>>,
    writer: Box<Write>,
}

impl WriteAmmo {
    pub fn to_stdout() -> Result<WriteAmmo, io::Error> {
        // TODO: very slow! Locks stdout for each write
        Ok(WriteAmmo {buff: io::Cursor::new(vec![]), writer: Box::new(StdoutWriter)})
    }

    pub fn to_file(filename: &Path) -> Result<WriteAmmo, io::Error> {
        let f = try!(File::create(filename));
        WriteAmmo::to_stream(Box::new(f))
    }

    pub fn to_gzip(filename: &Path) -> Result<WriteAmmo, io::Error> {
        let gz_command = format!("gzip -c > {}", filename.to_str().unwrap_or(""));
        let p = Box::new(try!(Command::new("sh")
            .arg("-c")
            .arg(gz_command)
            .stdin(Stdio::piped())
            .spawn()));
        Ok(WriteAmmo {buff: io::Cursor::new(vec![]), writer: Box::new(ProcWriter{child: p})} )
    }

    pub fn to_stream(to: Box<Write>) -> Result<WriteAmmo, io::Error> {
        let writer = BufWriter::new(to);
        Ok(WriteAmmo {buff: io::Cursor::new(vec![]), writer: Box::new(writer)} )
    }
}

impl AmmoProcessor for WriteAmmo {
    fn process(&mut self, bullet: &BulletData) -> Result<(), ProcError> {
        self.buff.set_position(0);
        self.buff.get_mut().clear();
        write_bullet(bullet, &mut self.buff, &mut self.writer)?;
        Ok(())
    }
}

struct ProcWriter {
    child: Box<Child>,
}

impl Write for ProcWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let stdin = self.child.stdin.as_mut();
        stdin.unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let stdin = self.child.stdin.as_mut();
        stdin.unwrap().flush()
    }
}

struct StdoutWriter;

impl Write for StdoutWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::stdout().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}
