extern crate reqwest;
extern crate anyhow;
extern crate serde_json;

// external
use http::{HeaderMap, HeaderValue, header::{COOKIE}};

// local
use crate::message::{InboundMessage, MessageQueue, OutboundMessage, PostResult};
use crate::commands::{Command, CommandSet};

#[derive(Debug)]
struct BotConfiguration {
    pub name: String,
    pub trip: String
}

impl BotConfiguration {
    pub async fn init(name: String, trip: String) -> Result<Self, anyhow::Error> {
        Ok(Self {
            name: name,
            trip: trip,
        })
    }
}

#[derive(Debug)]
pub struct ChanConnection {
    pub client: reqwest::Client,
    config: BotConfiguration,
    pub lastpost: u32, // i really hope the 4294967295 will be enough lmao
    limit: u8,
    raw_get_url: String,
    queue: MessageQueue,
    pub post_url: String,
    pub anna_cookie: String,
    commands: CommandSet,
    /*
        TODO: implement a way to store a set of outbound messages (as InboundMessage)
        Would be great for the API to properly function first i guess else it's gonna be fugly

        btw isnt this already implemented?
    */
}


impl ChanConnection {
    pub async fn init(
        anna_cookie: String,
        get_url: String,
        post_url: String,
        name: String,
        trip: String,
    ) -> Result<Self, anyhow::Error> {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .build()?;
        let queue = MessageQueue::init().await?;

        let config = BotConfiguration::init(name, trip).await?;

        let commands = CommandSet::init().await?;

        return Ok(Self {
            client: client,
            config: config,
            lastpost: 0u32,
            limit: 1u8,
            queue: queue,
            raw_get_url: get_url,
            post_url: post_url,
            anna_cookie: anna_cookie,
            commands: commands,
        })
    }

    pub fn set_lastpost(&mut self, latest: u32) {
        self.lastpost = latest;
    }

    // why cant this be something like pyhton @property tho
    pub fn get_url(&self) -> String {
        let mut result = format!("{}?count={}", self.raw_get_url, self.limit);
        if self.lastpost != 0u32 {
            result = format!("{}?count={}", result, self.lastpost);
        }
        println!("{}", result);
        return result;
    }

    pub fn construct_reply_text(&self, text: String, to: Option<u32>) -> String {
        let postnumber = match to {
            Some(number) => number,
            None => self.lastpost
        };
        return format!(">>{}\n{}", postnumber, text);
    }

    pub async fn add_to_queue(&mut self, message: InboundMessage) -> Result<(), anyhow::Error> {
        //  TODO: check for messages in the outbound history
        let is_bot = self.queue.check_if_outbound(message.clone()).await?;
        self.lastpost = message.count;
        self.queue.add_to_queue(message.clone(), is_bot).await?;
        println!("Message {:#?} is a bot message: {:#?}", message, is_bot);
        if !is_bot {
            match self.commands.check_against_commands(message.clone().body) {
                Some (reply_text) => {
                    let new_message = self.construct_reply(message, reply_text);
                    self.add_to_outbound_queue(new_message).await?;
                    return Ok(());
                },
                _ => {}
            }
        }
        Ok(())
    }

    pub fn construct_reply(&self, message: InboundMessage, raw_text: String) -> OutboundMessage {
        // construct a reply for an outbound message
        return OutboundMessage {
            chat: message.chat,
            name: Some(self.config.name.clone()),
            trip: Some(self.config.trip.clone()),
            body: self.construct_reply_text(raw_text, Some(message.count)),
            convo: message.convo,
        };
    }

    pub async fn process_messages(&mut self, messages: Vec<InboundMessage>) -> Result<(), anyhow::Error> {
        // TODO: implement
        for message in messages {
            self.add_to_queue(message).await?;
        }
        Ok(())
    }

    pub fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers
            .insert(
                COOKIE,
                HeaderValue::from_str(&format!("password_livechan={}", self.anna_cookie)).unwrap()
            );

        headers
            .insert(
                COOKIE,
                HeaderValue::from_str(&format!("nolimitcookie={}", self.anna_cookie)).unwrap()
            );
        return headers;
    }

    pub async fn add_to_outbound_queue(&mut self, message: OutboundMessage) -> Result<(), anyhow::Error> {
        self.queue.add_to_outbound_queue(message).await?;
        Ok(())
    }

    pub async fn send_message(&self, message: OutboundMessage) -> Result<bool, anyhow::Error> {
        let serialized_message = serde_json::json!(&message);

        let response = self.client
            .post(&self.post_url)
            // .post("https://jsonplaceholder.typicode.com/posts")
            .headers(self.headers())
            // .form(&serialized_message)
            .json(&serialized_message)
            .send()
            .await?
            .text()
            .await?;

        let post_result: PostResult = serde_json::from_str(&response)?;
        Ok(post_result.failed_to_send())
    }

    pub async fn attempt_sending_outbound(&mut self) -> Result<(), anyhow::Error> {
        match self.queue.first_to_send() {
            Some(message) => {
                println!("Sending: {:?}", message);
                let result: bool = self.send_message(message.clone()).await?;
                match result {
                    false => {
                        self.queue.append_as_first(message);
                    },
                    _ => return Ok(())
                }
                return Ok(());
            },
            None => {
                return Ok(());
            }
        }
    }

    pub async fn get_and_process_messages(&mut self) -> Result<(), anyhow::Error> {
        let response = &self.client
            .get(&self.get_url())
            .headers(self.headers())
            .send()
            .await?
            .text()
            .await?;
            
        let messages: Vec<InboundMessage> = serde_json::from_str(&response).unwrap();

        self.process_messages(messages).await?;
        Ok(())
    }
}