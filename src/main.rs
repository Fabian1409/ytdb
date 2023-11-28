use anyhow::Result;
use clap::{arg, command};
use image::open;
use rayon::prelude::*;
use std::{
    fs::{self, read_to_string, remove_file},
    process::{Command, Stdio},
};
use tempfile::tempdir;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const N_PIXEL: u32 = 4;

fn main() -> Result<()> {
    let matches = command!()
        .subcommand_required(true)
        .subcommand(clap::Command::new("get").arg(arg!(<key> "key of value to get")))
        .subcommand(
            clap::Command::new("insert")
                .arg(arg!(<key> "key of new entry"))
                .arg(arg!(<value> "value of new entry")),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("get", sub_m)) => {
            let key = sub_m.get_one::<String>("key").unwrap();

            let dir = tempdir()?;

            // ffmpeg -i output.mp4 -r 30 output_%d.png
            let mut ffmpeg = Command::new("ffmpeg")
                .args([
                    "-i",
                    format!("{key}.mp4").as_str(),
                    "-r",
                    "30",
                    format!("{}/%d.png", dir.path().to_str().unwrap()).as_str(),
                ])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .spawn()?;
            ffmpeg.wait()?;

            let mut bytes: Vec<Vec<u8>> = Vec::new();

            let mut frames: Vec<_> = fs::read_dir(dir.path().to_str().unwrap().to_owned() + "/")?
                .map(|f| f.unwrap())
                .collect();
            frames.reverse();

            frames
                .into_par_iter()
                .map(|file| {
                    let mut x = 0u32;
                    let mut y = 0u32;
                    let img = open(file.path()).unwrap().into_rgba8();
                    let mut frame_bytes = vec![];
                    loop {
                        let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                        for xp in x..x + N_PIXEL {
                            for yp in y..y + N_PIXEL {
                                let pixel = img.get_pixel(xp, yp);
                                r += *pixel.0.first().unwrap() as u32;
                                g += *pixel.0.get(1).unwrap() as u32;
                                b += *pixel.0.get(2).unwrap() as u32;
                            }
                        }

                        r /= N_PIXEL * N_PIXEL;
                        g /= N_PIXEL * N_PIXEL;
                        b /= N_PIXEL * N_PIXEL;

                        frame_bytes.push(r as u8);
                        frame_bytes.push(g as u8);
                        frame_bytes.push(b as u8);

                        if (r == 0) | (g == 0) | (b == 0) {
                            break;
                        }

                        x += N_PIXEL;

                        if x == WIDTH {
                            x = 0;
                            y += N_PIXEL;
                        }

                        if y == HEIGHT {
                            break;
                        }
                    }
                    frame_bytes
                })
                .collect_into_vec(&mut bytes);

            let value = String::from_utf8(bytes.into_iter().flatten().collect())?;
            fs::write(format!("{key}.txt"), value)?;
            // println!("{value}");
        }
        Some(("insert", sub_m)) => {
            let key = sub_m.get_one::<String>("key").unwrap();
            let value = sub_m.get_one::<String>("value").unwrap();

            let mut bytes: Vec<u8> = if let Ok(data) = read_to_string(value) {
                data.bytes().collect()
            } else {
                value.bytes().collect()
            };

            let dir = tempdir()?;

            let padding = 3 - bytes.len() % 3;
            bytes.resize(bytes.len() + padding, 0);

            bytes
                .par_chunks(((HEIGHT * WIDTH * 3) / (N_PIXEL * N_PIXEL)) as usize)
                .enumerate()
                .for_each(|(i, frame)| {
                    let mut img = image::ImageBuffer::new(WIDTH, HEIGHT);
                    let mut x = 0u32;
                    let mut y = 0u32;
                    for chunk in frame.chunks_exact(3) {
                        let r = chunk[0];
                        let g = chunk[1];
                        let b = chunk[2];

                        for xp in x..x + N_PIXEL {
                            for yp in y..y + N_PIXEL {
                                img.put_pixel(xp, yp, image::Rgb([r, g, b]));
                            }
                        }

                        x += N_PIXEL;

                        if x == WIDTH {
                            x = 0;
                            y += N_PIXEL;
                        }
                    }
                    img.save(format!("{}/{}.png", dir.path().to_str().unwrap(), i + 1))
                        .unwrap();
                });

            let _ = remove_file(format!("{key}.mp4"));

            // ffmpeg -framerate 30 -i key_%d.png -c:v libx264rgb -crf 0 -r 30 output.mp4
            let mut ffmpeg = Command::new("ffmpeg")
                .args([
                    "-framerate",
                    "30",
                    "-i",
                    format!("{}/%d.png", dir.path().to_str().unwrap()).as_str(),
                    "-c:v",
                    "libx264rgb",
                    "-crf",
                    "0",
                    "-r",
                    "30",
                    format!("{key}.mp4").as_str(),
                ])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .spawn()?;
            ffmpeg.wait()?;
        }
        _ => panic!("invalid cmd"),
    }
    Ok(())
}
