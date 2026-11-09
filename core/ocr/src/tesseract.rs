//! Linux: the `tesseract` program, when it is installed. The picture goes in on its input and the
//! words come back as a table (TSV) with their places, gathered here into lines. It is given a
//! few seconds, and nothing it is given is kept on disk.

use crate::prepare::Gray;
use crate::{Line, OcrError};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(20);

/// How tesseract looks at the picture on each try: a block of text, a whole page, loose words.
fn page_mode(attempt: u32) -> &'static str {
    match attempt {
        0 => "6",
        1 => "3",
        _ => "11",
    }
}

pub fn read(image: &Gray, attempt: u32) -> Result<Vec<Line>, OcrError> {
    let mut child = Command::new("tesseract")
        .args(["stdin", "stdout", "--psm", page_mode(attempt), "tsv"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                OcrError::NoReader(
                    "To read text on Linux, install tesseract (for example: sudo apt install tesseract-ocr), then try again".into(),
                )
            } else {
                OcrError::Failed(e.to_string())
            }
        })?;
    let png = image.to_png();
    let mut stdin = child.stdin.take().expect("a piped input");
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&png);
    });
    let mut stdout = child.stdout.take().expect("a piped output");
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let started = Instant::now();
    let status = loop {
        match child
            .try_wait()
            .map_err(|e| OcrError::Failed(e.to_string()))?
        {
            Some(s) => break s,
            None if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OcrError::Failed("tesseract took too long".into()));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        return Err(OcrError::Failed(
            err.lines().last().unwrap_or("tesseract failed").to_string(),
        ));
    }
    Ok(lines_of(&out))
}

/// The words of tesseract's table gathered into lines, each with the box around its words.
pub(crate) fn lines_of(tsv: &str) -> Vec<Line> {
    let mut out: Vec<(String, Line)> = Vec::new();
    for row in tsv.lines().skip(1) {
        let c: Vec<&str> = row.split('\t').collect();
        // level page block paragraph line word left top width height confidence text
        if c.len() < 12 || c[0] != "5" {
            continue;
        }
        let text = c[11].trim();
        let conf: f32 = c[10].parse().unwrap_or(-1.0);
        if text.is_empty() || conf < 0.0 {
            continue;
        }
        let num = |i: usize| c[i].parse::<u32>().unwrap_or(0);
        let key = format!("{}/{}/{}/{}", c[1], c[2], c[3], c[4]);
        let (x, y, w, h) = (num(6), num(7), num(8), num(9));
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some((_, l)) => {
                let right = (l.x + l.w).max(x + w);
                let bottom = (l.y + l.h).max(y + h);
                l.x = l.x.min(x);
                l.y = l.y.min(y);
                l.w = right - l.x;
                l.h = bottom - l.y;
                l.text.push(' ');
                l.text.push_str(text);
            }
            None => out.push((
                key,
                Line {
                    text: text.to_string(),
                    x,
                    y,
                    w,
                    h,
                },
            )),
        }
    }
    out.into_iter().map(|(_, l)| l).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_gathered_into_their_lines() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t100\t40\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t5\t30\t12\t96.1\tHello\n\
                   5\t1\t1\t1\t1\t2\t45\t4\t40\t14\t95.0\tworld\n\
                   5\t1\t1\t1\t2\t1\t10\t25\t20\t12\t90.0\tagain\n\
                   5\t1\t1\t1\t2\t2\t40\t25\t20\t12\t-1\t \n";
        assert_eq!(
            lines_of(tsv),
            vec![
                Line {
                    text: "Hello world".into(),
                    x: 10,
                    y: 4,
                    w: 75,
                    h: 14
                },
                Line {
                    text: "again".into(),
                    x: 10,
                    y: 25,
                    w: 20,
                    h: 12
                },
            ]
        );
    }
}
