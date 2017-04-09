#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_derive)]

extern crate term;
extern crate qrcode;
extern crate image;
extern crate clap;
extern crate error_chain;
#[macro_use]
extern crate derive_error_chain;
extern crate get_if_addrs;
#[macro_use]
extern crate lazy_static;
extern crate rocket;
extern crate rocket_contrib;

mod error;
mod qrcode_local;

use std::path::Path;
use std::fs::File;
use std::io::{Read, BufRead, Write, stdin, stdout, stderr};
use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::collections::HashMap;

use clap::{Arg, App, SubCommand};
use rocket::response::NamedFile;
use rocket::config::{Config, Environment};
use rocket::{Request, Data};
use rocket_contrib::Template;

use error::Result;
use qrcode_local::render_and_print_qr_code;

lazy_static! {
    pub static ref INPUT: Mutex<String> = {
            Mutex::new(String::new())
    };
}

lazy_static! {
    pub static ref SERVER_ADDR: Mutex<String> = {
            Mutex::new(String::new())
    };
}

fn main() {
    if let Err(e) = _main() {
        writeln!(stderr(), "Oops! {}", e);
        std::process::exit(1)
    }
}

// -> 解析启动参数
//  -> send
//      -> 判断是否指定了 type
//          -> 是
//              -> -type
//                  -> text
//                      -> 判断是否超出大小
//                          -> 是 -> 询问 -> 是 -> 否: you may zoom in
//                          -> 否 ->
//                  -> file/dir
//                      -> --host, --port
//          -> 否
//              -> 判断是否 input 是一个有效 file/dir path
//                  -> 是
//                      -> 判断是否特地提供了 --host
//                          -> 是
//                              -> file/dir
//                          -> 否
//                              -> 询问 type
//                  -> 否
//                      -> text
//  -> receive
//      --host, --port
fn _main() -> Result<()> {
    let matches = App::new("qrs")
        .about("Sharing via QR Code")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(SubCommand::with_name("send")
            .about("Send text or file(s)")
            .arg(Arg::with_name("type")
                .short("t")
                .long("type")
                .help("Type of stuff to send")
                .takes_value(true)
                .possible_values(&["text", "file"])
                .value_name("TYPE"))
            .arg(Arg::with_name("max_qr_len")
                .short("m")
                .long("max-len")
                .help("The max length of QR Code")
                .takes_value(true)
                .default_value("120")
                .value_name("MAX_QR_LEN"))
            .arg(Arg::with_name("host")
                .short("h")
                .long("host")
                .help("Host to bind")
                .takes_value(true)
                .value_name("HOST"))
            .arg(Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to bind")
                .takes_value(true)
                .default_value("4141")
                .value_name("PORT"))
            .arg(Arg::with_name("input")
                .help("Stuff to send, text or file system path")
                .value_name("INPUT")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("receive")
            .about("Receive text or file(s)")
            .arg(Arg::with_name("host")
                .short("h")
                .long("host")
                .help("Host to bind")
                .takes_value(true)
                .value_name("HOST"))
            .arg(Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to bind")
                .takes_value(true)
                .default_value("4141")
                .value_name("PORT")))
        .get_matches();

    match matches.subcommand() {
        ("send", Some(submatch)) => {
            let input = submatch.value_of("input").ok_or("input required")?;
            let typ = match submatch.value_of("type") {
                Some("text") => Input::Text(input),
                Some("file") => {
                    let meta = std::fs::metadata(input)?;
                    if meta.is_dir() {
                        Input::Dir(input)
                    } else {
                        Input::File(input)
                    }
                }
                _ => {
                    let meta_ret = std::fs::metadata(input);
                    match meta_ret {
                        Ok(meta) => {
                            // 判断是否提供了 host, 是则为 Input::Dir/File
                            if let Some(_) = submatch.value_of("host") {
                                if meta.is_dir() {
                                    Input::Dir(input)
                                } else {
                                    Input::File(input)
                                }
                            } else {
                                // 询问是否想要分享为文件
                                if terminal_dialog_quiz("INPUT seems to be a path, share as \
                                                         file(s)?")? {
                                    if meta.is_dir() {
                                        Input::Dir(input)
                                    } else {
                                        Input::File(input)
                                    }
                                } else {
                                    Input::Text(input)
                                }
                            }
                        }
                        _ => Input::Text(input),
                    }
                }
            };
            match typ {
                Input::Text(input) => {
                    let max_qr_len: usize = submatch.value_of("max_qr_len").ok_or("")?.parse()?;
                    if input.len() > max_qr_len {
                        // 询问是否通过网络而非二维码分享文本信息
                        if terminal_dialog_quiz("INPUT seems to be too long, sharing via \
                                                 network?")? {
                            *INPUT.lock()? = input.to_string();
                            let conf = rocket_preheat(submatch)?;
                            rocket::custom(conf, true).mount("/", routes![send_text]).launch()
                        } else {
                            qrcode_local::render_and_print_qr_code(input)?;
                            println!("you may zoom out the terminal :)")
                        }
                    } else {
                        qrcode_local::render_and_print_qr_code(input)?
                    }
                }
                Input::File(input) => {
                    *INPUT.lock()? = input.to_string();
                    let conf = rocket_preheat(submatch)?;
                    rocket::custom(conf, true).mount("/", routes![send_singleton_file]).launch()
                }
                Input::Dir(input) => {
                    *INPUT.lock()? = input.to_string();
                    let conf = rocket_preheat(submatch)?;
                    rocket::custom(conf, true)
                        .mount("/", routes![send_dir_detail, send_dir_file])
                        .launch()
                }
            }
        }
        ("receive", Some(submatch)) => {
            let conf = rocket_preheat(submatch)?;
            rocket::custom(conf, true)
                .mount("/", routes![receive_index, receive_text])
                .mount("/",
                       vec![rocket::Route::from(&rocket::StaticRouteInfo {
                                method: rocket::http::Method::Post,
                                path: "/file",
                                handler: receive_file,
                                format: None,
                                rank: None,
                            })])
                .launch()
        }
        _ => unimplemented!(),
    }
    Ok(())
}

#[get("/")]
fn send_dir_detail() -> Result<Template> {
    let ref addr = *SERVER_ADDR.lock()?;
    let ref dir = *INPUT.lock()?;

    let mut file_urls = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let file_name = entry?
                .file_name()
                .into_string()
                .map_err(|_| "lossy string from OsString")?;
        file_urls.push(format!("{}{}", addr, file_name));
    }

    let mut data = HashMap::new();
    data.insert("files", file_urls);
    Ok(Template::render("send_dir_detail", &data))
}

#[get("/<file>")]
fn send_dir_file(file: String) -> Result<NamedFile> {
    let ref dir = *INPUT.lock()?;
    Ok(NamedFile::open(std::path::Path::new(dir).join(file))?)
}

#[get("/")]
fn send_singleton_file() -> Result<NamedFile> {
    Ok(NamedFile::open(INPUT.lock()?.as_str())?)
}

#[get("/")]
fn send_text() -> Result<String> {
    Ok(INPUT.lock()?.clone())
}

#[get("/")]
fn receive_index() -> Template {
    Template::render("receive", &"")
}

fn receive_file<'b>(req: &'b Request, data: Data) -> rocket::handler::Outcome<'b> {
    let _receive_file = || -> Result<&str> {
        //  ------ServoFormBoundaryqs9cjXKlH2fyGIAM
        //  Content-Disposition: form-data; name="file"; filename="rust.logo"
        //  Content-Type: image/jpeg
        //
        //  <FILE_CONTENT>
        //  ------ServoFormBoundaryqs9cjXKlH2fyGIAM--
        //
        let content_length: usize = req.headers().get("Content-Length").next().ok_or("")?.parse()?;
        let boundary = req.headers()
            .get("Content-Type")
            .next()
            .ok_or("")?
            .rsplit("boundary=")
            .next()
            .ok_or("")?;

        let mut has_read = 0;
        let mut file_name = String::new();
        let mut reader = data.open();
        loop {
            let mut buf = String::new();
            reader.read_line(&mut buf)?;

            let buf_len = buf.len();
            has_read += buf_len;

            if buf.starts_with("Content-Disposition") {
                let ref str_end_with_filename = buf[..buf_len - 3];
                let start = str_end_with_filename.rfind("\"").ok_or("")?;
                file_name = str_end_with_filename[start + 1..].to_string();
            }
            if buf == "\r\n" {
                break;
            }
        }

        // save to file ->
        // if ./file_name exist => file_name = ./file_name.{integer}
        let path = Path::new(&file_name);
        let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();
        let mut path_buf = path.to_path_buf();
        let mut file_name_extra = 0;
        while path_buf.exists() {
            file_name_extra += 1;
            path_buf = path.with_extension(if ext.len() == 0 {
                format!("{}", file_name_extra)
            } else {
                format!("{}.{}", ext, file_name_extra)
            });
        }

        let mut file = File::create(path_buf)?;
        let file_len = content_length - has_read - 2 - (boundary.len() + 4) - 2;
        let mut has_been_read = 0;
        loop {
            let ref mut buf = [0; 32 * 1024];
            let n = reader.read(buf)?;
            if n == 0 {
                break;
            }
            has_been_read += n;

            if has_been_read > file_len {
                file.write_all(&buf[..file_len - (has_been_read - n)])?;
                break;
            } else {
                file.write_all(&buf[..n])?;
            }
        }
        Ok("done!")
    };
    rocket::handler::Outcome::of(_receive_file())
}

#[derive(FromForm)]
struct Text {
    text: String,
}

#[post("/text", data = "<data>")]
fn receive_text(data: rocket::request::Form<Text>) -> &'static str {
    println!(r"================
{}
================",
             data.get().text);
    "done!"
}

/// Input from env::args
pub enum Input<'a> {
    Text(&'a str),
    File(&'a str),
    Dir(&'a str),
}

fn select_iface_ip_to_bind() -> Result<Ipv4Addr> {
    use get_if_addrs::{IfAddr, Ifv4Addr};

    let ifaces = get_if_addrs::get_if_addrs()?;
    let ipv4s: Vec<Ipv4Addr> = ifaces.iter()
        .filter_map(|i| if i.is_loopback() {
            None
        } else if let IfAddr::V4(Ifv4Addr { ip, .. }) = i.addr {
            Some(ip)
        } else {
            None
        })
        .collect();

    match ipv4s.len() {
        0 => Err("none suitable interface to bind on")?,
        1 => Ok(ipv4s[0]),
        _ => {
            loop {
                println!("Which interface do you prefer?\n");
                for (i, ip) in ipv4s.iter().enumerate() {
                    println!("{}): {}", i, ip);
                }

                let mut buf = String::new();
                stdin().read_line(&mut buf)?;

                let choice: usize = buf.trim().parse()?;
                if choice < ipv4s.len() {
                    return Ok(ipv4s[choice]);
                } else {
                    continue;
                }
            }
        }
    }
}

fn terminal_dialog_quiz<T: AsRef<[u8]>>(print: T) -> Result<bool> {
    loop {
        stdout().write_all(print.as_ref())?;
        stdout().write_all(" [Y/n]".as_bytes())?;
        stdout().flush()?;

        let mut buf = String::with_capacity(2);
        stdin().read_line(&mut buf)?;

        match buf.as_str() {
            "\n" | "Y\n" | "y\n" => return Ok(true),
            "N\n" | "n\n" => return Ok(false),
            _ => continue,
        }
    }
}

fn meet_rocket_config(clap_arg: &clap::ArgMatches) -> Result<Config> {
    // 判断是否提供了 --host
    //  -> 是 -> done!
    //  -> 否 -> select_iface_ip_to_bind()
    let host = if let Some(host) = clap_arg.value_of("host") {
        host.to_string()
    } else {
        select_iface_ip_to_bind()?.to_string()
    };
    let port: u16 = clap_arg.value_of("port").ok_or("no port")?.parse()?;

    let conf = Config::build(Environment::Staging)
        .address(host)
        .port(port)
        .unwrap();
    Ok(conf)
}

fn rocket_preheat(clap_arg: &clap::ArgMatches) -> Result<Config> {
    let conf = meet_rocket_config(clap_arg)?;
    let addr = format!("http://{}:{}/", conf.address, conf.port);
    *SERVER_ADDR.lock()? = addr.clone();
    render_and_print_qr_code(addr)?;

    Ok(conf)
}
