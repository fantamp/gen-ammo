extern crate rand;
extern crate logut;
extern crate clap;
extern crate twoway;
use std::path::{Path, PathBuf};
use std::io;
use clap::{Arg, App};
use logut::*;
mod ammo;
mod error;
mod ammo_proc;
use ammo_proc::AmmoProcessor;
use logut::read::{ReadByLine};

#[derive(PartialEq)]
enum Algo {
    ReserviorSampling,
    MethodS,
    DoNotRandomize,
}

impl Default for Algo {
    fn default() -> Self {
        Algo::DoNotRandomize
    }
}

type ReaderFabric = Fn() -> Box<ReadByLine>;

pub enum LinesSource {
    FileName(PathBuf),
    Fabric(Box<ReaderFabric>),
}

#[derive(Default)]
struct RunConf {
    in_files: Vec<LinesSource>,
    out_files: Vec<PathBuf>,
    algo: Algo,
    target_set_size: Option<usize>,
}

fn get_conf_from_cli(args: Option<Vec<&'static str>>) -> RunConf {
    let ver = option_env!("CARGO_PKG_VERSION");

    fn is_int(v: String) -> Result<(), String> {
        match v.parse::<usize>() {
            Ok(_) => Ok(()),
            Err(_) => Err("not a number".to_string())
        }
    }

    fn is_greater_than_zero(v: String) -> Result<(), String> {
        match is_int(v.clone()) {
            Err(s) => Err(s),
            Ok(_) => if v.parse::<usize>().unwrap() > 0 {
                    Ok(())
                } else {
                    Err("value must be greater than zero".to_string())
                }
        }
    }

    let app = App::new("Ammo Generator")
        .version(ver.unwrap_or("unknown"))
        .author("Andrey Mescheryakov")
        .arg(
            Arg::with_name("method")
                .short("m")
                .long("method")
                .takes_value(true)
                .possible_values(&["stream", "inmem"])
                .requires("count")
                .help("Mixing method"))
        .arg(
            Arg::with_name("in")
                .short("i")
                .long("in")
                .takes_value(true)
                .multiple(true)
                .help("Use these files as input (you may specify more than one)"))
        .arg(
            Arg::with_name("out")
                .short("o")
                .long("out")
                .takes_value(true)
                .multiple(true)
                .conflicts_with_all(&["nfiles", "ammo_prefix"])
                .help("Write ammo in these files"))
        .arg(
            Arg::with_name("ammo_prefix")
                .short("p")
                .long("ammo-prefix")
                .takes_value(true)
                .requires("nfiles")
                .conflicts_with("out")
                .help("Create output files with this prefix. E.g. '... -p /home/fantamp/ammo/20170103- -n 2' will create two files: /home/fantamp/ammo/20170103-01.gz /home/fantamp/ammo/20170103-02.gz"))
        .arg(
            Arg::with_name("gzip")
                .short("g")
                .long("gzip")
                .requires("ammo_prefix")
                .help("Gzip output files (and use .gz extension for them)"))
        .arg(
            Arg::with_name("nfiles")
                .short("n")
                .long("nfiles")
                .takes_value(true)
                .validator(is_greater_than_zero)
                .requires("ammo_prefix")
                .conflicts_with("out")
                .help("Count of output files"))
        .arg(
            Arg::with_name("count")
                .short("c")
                .long("count")
                .takes_value(true)
                .validator(is_int)
                .help("Write COUNT bullets to each output file"));

    let matches = match args {
        None => app.get_matches(),
        Some(v) => app.get_matches_from(v)
    };

    let method = match matches.value_of("method") {
        Some("stream") => Algo::MethodS,
        Some("inmem") => Algo::ReserviorSampling,
        None => Algo::DoNotRandomize,
        _ => panic!("unknown mixing algorithm"),
    };

    fn get_files(m: &clap::ArgMatches, opt: &str) -> Vec<PathBuf> {
        match m.values_of(opt) {
           None => Vec::new(),
           Some(it) => it.map(|x|Path::new(x).to_path_buf()).collect(),
       }
    }

    let gzip_output = matches.is_present("gzip");

    let in_files = get_files(&matches, "in");
    let out_files = match matches.value_of("nfiles") {
        None => get_files(&matches, "out"),
        Some(s) => {
            let files_count = s.parse::<usize>().unwrap();
            let prefix = matches.value_of("ammo_prefix").unwrap_or("");
            let ext = if gzip_output {"gz"} else {"txt"};
            (0..files_count).map(|x| Path::new(&format!("{}-{:02}.{}", prefix, x, ext)).to_path_buf()).collect::<Vec<PathBuf>>()
        }
    };

    let target_set_size = matches.value_of("count").map(|s| {
        let count = s.parse::<usize>().unwrap();
        let nfiles = out_files.len();
        count * nfiles
    });

    RunConf {
        in_files: in_files.iter().map(|x| LinesSource::FileName(x.clone())).collect(),
        out_files: out_files,
        algo: method,
        target_set_size: target_set_size,
    }
}

fn make_writer(conf: &RunConf) -> Result<Box<AmmoProcessor>, error::ProcError> {
    use std::ffi::OsStr;
    let mut writers: Vec<Box<ammo_proc::AmmoProcessor>> = Vec::new();
    if conf.out_files.len() <= 0 {
        writers.push(Box::new(ammo_proc::WriteAmmo::to_stdout()?));
    } else {
        for path in &conf.out_files {
            let extension = path.extension().unwrap_or(OsStr::new(""));
            let writer = if extension == "gz" {
                Box::new(ammo_proc::WriteAmmo::to_gzip(path)?)
            } else {
                Box::new(ammo_proc::WriteAmmo::to_file(path)?)
            };
            writers.push(writer);
        }
    }
    Ok(Box::new(ammo_proc::RoundRobin::new(writers)))
}

struct FilteringReader {
    check: Box<Fn(&[u8]) -> bool>,
    source: Box<ReadByLine>,
}

impl ReadByLine for FilteringReader {
    fn process_lines(&mut self, feed_to: &mut FnMut(&[u8])) -> io::Result<()> {
        let closure = &self.check;
        let mut process_line = |line: &[u8]| {
            if (*closure)(line) {
                feed_to(line);
            }
        };
        self.source.process_lines(&mut process_line)
    }
}


fn make_reader(conf: &RunConf) -> Result<Box<ReadByLine>, std::io::Error> {
    let check_fn = |line: &[u8]| -> bool {
        twoway::find_bytes(line, b"rep-outgoing=1") == None &&
            twoway::find_bytes(line, b"subrequest=1") == None
    };
    let source: Box<ReadByLine> = if conf.in_files.len() <= 0 {
        Box::new(logut::read::FromStdin)
    } else {
        let mut readers: Vec<Box<ReadByLine>> = Vec::new();
        for source in &conf.in_files {
            let reader: Box<ReadByLine> = match source {
                &LinesSource::FileName(ref path) => {
                    if !path.is_file() {
                        return Err(io::Error::new(io::ErrorKind::NotFound, format!("Path {:?} not exists or it is not a file", path)));
                    }
                    Box::new(read::FileLinesReader{filename: path.clone()})
                },
                &LinesSource::Fabric(ref fabric) => { (*fabric)() }
            };
            readers.push(reader);
        }
        Box::new(logut::read::Chained{sources: readers})
    };

    Ok(Box::new(FilteringReader{check: Box::new(check_fn), source: source}))
}

fn get_lines_count(conf: &RunConf) -> std::io::Result<usize> {
    let mut count: usize = 0;
    make_reader(conf)?.process_lines(&mut |_| count += 1)?;
    Ok(count)
}

fn make_processor(conf: &RunConf, writer: Box<AmmoProcessor>) -> io::Result<Box<AmmoProcessor>> {
    let processor = match conf.algo {
        Algo::MethodS => {
            let lines_count = get_lines_count(conf)?;
            ammo_proc::MethodS::new(lines_count, conf.target_set_size.unwrap(), writer)
        },
        Algo::ReserviorSampling => Box::new(ammo_proc::ReserviorSampling::new(conf.target_set_size.unwrap(), writer)),
        Algo::DoNotRandomize => writer,
    };
    Ok(processor)
}

fn make_log_line_process_func<'a>(ammo_processor: &'a mut Box<AmmoProcessor>) -> Box<FnMut(&[u8]) + 'a> {
    let process_log_line = move |line_from_log: &[u8]| {
        let rec = parse_log_line(line_from_log);
        let bullet_data = ammo::make_bullet_data_from_log_record(rec);
        ammo_processor.process(&bullet_data).unwrap();
    };
    Box::new(process_log_line)
}

fn main() {
    let conf = get_conf_from_cli(None);
    let writer = make_writer(&conf).unwrap();
    let mut mixer = make_processor(&conf, writer).unwrap();

    {
        let mut reader = make_reader(&conf).unwrap();
        let mut f = make_log_line_process_func(&mut mixer);
        reader.process_lines(&mut *f).unwrap();
    }

    mixer.finish().unwrap();
}

#[cfg(test)]
mod tests {
    use super::Algo;
    use std::io::Cursor;
    use logut::read::*;
    use super::*;

    #[test]
    fn no_args() {
        let conf = super::get_conf_from_cli(Some(vec![]));
        assert!(conf.algo == Algo::DoNotRandomize);
        assert!(conf.target_set_size.is_none());
        assert!(conf.in_files.len() == 0);
        assert!(conf.out_files.len() == 0);
    }

    #[test]
    fn many_in_files() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--in", "file1.txt", "file2.txt", "file3.txt"]));
        assert!(conf.algo == Algo::DoNotRandomize);
        assert!(conf.target_set_size.is_none());
        assert!(conf.in_files.len() == 3);
        assert!(conf.out_files.len() == 0);
    }

    #[test]
    fn many_in_files2() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--in", "file1.txt", "--in", "file2.txt", "file3.txt"]));
        assert!(conf.algo == Algo::DoNotRandomize);
        assert!(conf.target_set_size.is_none());
        assert!(conf.in_files.len() == 3);
        assert!(conf.out_files.len() == 0);
    }

    #[test]
    fn many_out_files() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--out", "file1.gz", "file2.gz", "file3.gz"]));
        assert!(conf.algo == Algo::DoNotRandomize);
        assert!(conf.target_set_size.is_none());
        assert!(conf.in_files.len() == 0);
        assert!(conf.out_files.len() == 3);
    }

    #[test]
    fn gen_files_with_prefix_gzip() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--ammo-prefix", "file", "--nfiles", "3", "--gzip"]));
        assert!(conf.algo == Algo::DoNotRandomize);
        assert!(conf.target_set_size.is_none());
        assert!(conf.in_files.len() == 0);
        assert!(conf.out_files.len() == 3);
        assert_eq!(conf.out_files[0].to_str(), Some("file-00.gz"));
        assert_eq!(conf.out_files[1].to_str(), Some("file-01.gz"));
        assert_eq!(conf.out_files[2].to_str(), Some("file-02.gz"));
    }

    #[test]
    fn gen_files_with_prefix_no_gzip() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--ammo-prefix", "file", "--nfiles", "3"]));
        assert_eq!(conf.out_files[0].to_str(), Some("file-00.txt"));
        assert_eq!(conf.out_files[1].to_str(), Some("file-01.txt"));
        assert_eq!(conf.out_files[2].to_str(), Some("file-02.txt"));
    }

    #[test]
    fn in_mem_algo_conf() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--method", "inmem", "--count", "1000", "--in", "file1.txt", "--ammo-prefix", "file", "--nfiles", "3"]));
        assert!(conf.algo == Algo::ReserviorSampling);
        assert_eq!(conf.target_set_size.unwrap(), 3000);
    }

    #[test]
    fn stream_algo_conf() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--method", "stream", "--count", "1000", "--in", "file1.txt", "--ammo-prefix", "file", "--nfiles", "3"]));
        assert!(conf.algo == Algo::MethodS);
        assert_eq!(conf.target_set_size.unwrap(), 3000);
    }

    #[test]
    fn count_1() {
        let conf = super::get_conf_from_cli(Some(vec!["gen_ammo", "--method", "stream", "--count", "1000", "--in", "file1.txt", "--out", "file1", "file2", "file3"]));
        assert!(conf.algo == Algo::MethodS);
        assert_eq!(conf.target_set_size.unwrap(), 3000);
    }

    fn make_fabric(content: &str) -> LinesSource {
        let content = content.to_string();
        let closure = move || -> Box<ReadByLine> {
            let reader = GenericReader{
                reader: Box::new(Cursor::new(content.clone()))
            };
            Box::new(reader)
        };
        LinesSource::Fabric(Box::new(closure))
    }

    #[test]
    fn filter_aux_requests() {
        let content = "line one\nline two\nline three\nhttp://you.ru?subrequest=1\nhttp://example.com?subrequest=1\nrep-outgoing=1\nline six";

        let conf = super::RunConf {
            in_files: vec![make_fabric(content)],
            ..Default::default()
        };

        let mut lines: Vec<Vec<u8>> = Vec::new();
        let mut reader = super::make_reader(&conf).unwrap();
        reader.process_lines(&mut |line: &[u8]| lines.push(line.to_vec())).unwrap();

        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn simple_run() {
        let content = "one\ntwo\nthree";
        let conf = super::RunConf {
            in_files: vec![make_fabric(content)],
            ..Default::default()
        };
        super::make_reader(&conf).unwrap();
    }

    // TODO: deny combination of stdin and --method=stream
    // TODO: check that fails without --count
    // TODO: not in countd
    // TODO: zero count
    // TODO: nfiles excludes --out and vs
}
