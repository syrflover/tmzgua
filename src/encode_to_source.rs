use std::{io, process::Stdio};

use songbird::input::{Codec, Container, Input, Reader};
use tokio::{io::AsyncReadExt, process::Command};

pub async fn encode_to_source<T>(a: T) -> io::Result<Input>
where
    T: Into<Stdio>,
{
    let ffmpeg_args = [
        "-f",
        "s16le",
        "-ac",
        "2",
        "-ar",
        "48000",
        "-acodec",
        "pcm_f32le",
        "-",
    ];

    let mut ffmpeg = Command::new("ffmpeg")
        .arg("-i")
        .arg("-")
        .args(ffmpeg_args)
        .stdin(a)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut buf = Vec::new();
    let mut stdout = ffmpeg.stdout.take().unwrap();

    stdout.read_to_end(&mut buf).await?;

    let source = Input::new(
        true,
        Reader::from_memory(buf),
        Codec::FloatPcm,
        Container::Raw,
        None,
    );

    Ok(source)
}
