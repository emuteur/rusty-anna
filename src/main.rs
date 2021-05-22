use std::{env, os::linux::raw};
// use std::path::Path;
use tokio::time::{sleep, Duration};

extern crate dotenv;
extern crate reqwest;
extern crate serde_json;
extern crate anyhow;

// local modules
mod connection;
mod message;

// external crates
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();
    
    // anna_nolimit_cookie1
    let anna_cookie = env::var("ANNA_COOKIE")
        .expect("ANNA_COOKIE is not set in the .env file");
    // let anna_cookie = "";

    // kotchan.fun
    let domain = env::var("DOMAIN")
        .expect("DOMAIN is not set in the .env file");

    let board = env::var("BOARD")
        .expect("BOARD is not set in the .env file");

    let url = format!("https://{}/last/{}/", domain, board);
    let post_url = format!("https://{}/chat/{}/", domain, board);
    
    // TODO: load chat, name and trip from env variables and save in the connection

    let mut con = connection::ChanConnection::init(
        anna_cookie, url, post_url,
    ).await?;
    
    loop {
        let _greeting = message::OutboundMessage {
            chat: String::from("int"),
            name: Some(String::from("salobot")),
            trip: Some(String::from("test")),
            body: String::from("Connected to the chat."),
            convo: String::from("GeneralDEBUG"),
        };

    //  notify about successful connection
        con.add_to_outbound_queue(_greeting).await?;
        con.attempt_sending_outbound().await?;

        get_messages(&con).await?;
    }

}

async fn get_messages(con: &connection::ChanConnection) -> Result<(), anyhow::Error> {
    let response = con.client
        .get(&con.get_url())
        .headers(con.headers())
        .send()
        .await?
        .text()
        .await?;
        
    let messages: Vec<message::InboundMessage> = serde_json::from_str(&response).unwrap();

    con.process_messages(messages).await?;

    let plain_json: serde_json::Value = serde_json::from_str(&response)?;
    // println!("\n\n{:#?}", plain_json);
    // println!("\n\n\nfirst: {:#?}", messages[0]);
    
    sleep(Duration::from_millis(250)).await;
    Ok(())
}