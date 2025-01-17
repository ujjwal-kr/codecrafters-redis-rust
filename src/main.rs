use std::{
    io,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

mod commands;
mod decoder;
mod encoder;
mod storage;

use commands::*;
use decoder::*;
use encoder::*;
use storage::Storage;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:6379").await.unwrap();
    let storage_engine = Arc::new(Mutex::new(Storage::new()));
    println!("Listening on ::6379");
    loop {
        let incoming = listener.accept().await;
        let cloned_storage = Arc::clone(&storage_engine);
        match incoming {
            Ok((mut stream, addr)) => {
                println!("New Connection, {}", addr);
                tokio::spawn(async move {
                    handle_connection(&mut stream, cloned_storage).await;
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

async fn read_data(stream: &mut TcpStream) -> Result<Vec<u8>, io::Error> {
    const MAX_BUFFER_SIZE: usize = 512;
    let mut buf: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; MAX_BUFFER_SIZE];
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&buffer);
        if n < MAX_BUFFER_SIZE {
            break;
        }
    }
    Ok(buf)
}

async fn handle_connection(stream: &mut TcpStream, client_store: Arc<Mutex<Storage>>) {
    let mut buf: Vec<u8>;
    loop {
        match read_data(stream).await {
            Ok(b) => buf = b,
            Err(_) => break,
        }
        if buf.is_empty() {
            println!("Client closed the connection");
            break;
        }
        let str_cmd = String::from_utf8_lossy(&buf);
        let cmd: Vec<&str> = str_cmd.split("\r\n").collect::<Vec<&str>>();
        if let Ok(mut cmd_len) = cmd[0][1..2].parse::<usize>() {
            cmd_len *= 2;
            let pure_cmd = decode_get_pure_command(cmd[0..cmd_len + 1].to_vec());
            match pure_cmd[0].to_ascii_lowercase().trim() {
                "ping" => ping(stream).await,
                "echo" => echo(stream, pure_cmd).await,
                "set" => set(stream, pure_cmd, Arc::clone(&client_store)).await,
                "get" => get(stream, pure_cmd, Arc::clone(&client_store)).await,
                "del" => del(stream, pure_cmd, Arc::clone(&client_store)).await,
                "llen" => array::llen(stream, pure_cmd, Arc::clone(&client_store)).await,
                "lpop" => array::lpop(stream, pure_cmd, Arc::clone(&client_store)).await,
                "lrem" => array::lrem(stream, pure_cmd, Arc::clone(&client_store)).await,
                "lset" => array::lset(stream, pure_cmd, Arc::clone(&client_store)).await,
                "lpush" | "rpush" => array::push(stream, pure_cmd, Arc::clone(&client_store)).await,
                "lrange" => array::lrange(stream, pure_cmd, Arc::clone(&client_store)).await,
                "lindex" => array::lindex(stream, pure_cmd, Arc::clone(&client_store)).await,
                "hset" => hash::hash_set(stream, pure_cmd, Arc::clone(&client_store)).await,
                "qadd" => queue::queue_add(stream, pure_cmd, Arc::clone(&client_store)).await,
                "qread" => queue::dequeue(stream, pure_cmd, Arc::clone(&client_store)).await,
                "qlen" => queue::qlen(stream, pure_cmd, Arc::clone(&client_store)).await,
                _ => undefined(stream).await,
            };
            buf.clear();
        } else {
            stream
                .write(&encode_resp_error_string("Error in parsing cmd length"))
                .await
                .unwrap();
        }
    }
}
