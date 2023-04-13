use std::fmt;

use tokio::sync::mpsc::Sender;
use rumqttc::{Incoming, EventLoop, SubscribeFilter};
use rumqttc::{MqttOptions, AsyncClient, qos as qos_make, Event, ClientError, ConnectionError, mqttbytes::Error as MqttBytesError};

use crate::actor::dist::{Signal as SignalDist, Payload as PayloadDist};
use crate::config::ConfigMqttClient;


pub struct Sub {
    is_active: bool,
    is_online: bool,
    id: u32,
    client: AsyncClient,
    eventloop: EventLoop,
    tx_dist: Sender<SignalDist>,
    topic_vec: Option<Vec<(String, u8)>>,
}

impl Sub {
    pub fn new(id: u32, config: ConfigMqttClient, topic_vec: Vec<(String, u8)>, tx_dist: Sender<SignalDist>) -> Self {
        let mut options = MqttOptions::new(config.id, config.host, config.port);
        options.set_clean_session(config.clean_session);
        options.set_keep_alive(config.keep_alive);
        let (client, eventloop) = AsyncClient::new(options, config.capacity); 

        Self{
            id, client, eventloop, tx_dist,
            is_online: false,
            is_active: true,
            topic_vec: Some(topic_vec),
        }
    }

    async fn close(&mut self) {
        self.is_active = false;
        let _ = self.client.disconnect().await;
    }

    async fn destruct(&mut self) {
        let _ = self.send_dist(PayloadDist::Closed).await;
    }

    async fn send_dist(&mut self, payload: PayloadDist) -> Result<(), ()> {
        if let Err(_) = self.tx_dist.send(SignalDist{id_broker: self.id, payload}).await {
            self.close().await;
            Err(())
        } else {
            Ok(())
        }
    }

    fn init(&mut self) -> Result<(), SubError> {
        let topic_vec = self.topic_vec.as_ref().ok_or_else(|| SubError::TopicsMissing)?;
        let mut sub_filter_vec: Vec<SubscribeFilter> = Vec::with_capacity(topic_vec.len());
        for (topic, qos_u8) in topic_vec {
            let qos = qos_make(qos_u8.to_owned()).map_err(|err| SubError::QosMapping(err))?;
            sub_filter_vec.push(SubscribeFilter { path: topic.clone(), qos: qos });
        }
        self.client.try_subscribe_many(sub_filter_vec).map_err(|err| SubError::TopicsSubscribe(err))?;
        Ok(())
    }

    pub async fn serve(&mut self) {
        'outer: loop {
            loop {
                if !self.is_active { break 'outer; }
                if let Err(err) = self.init() {
                    println!("sub init error {}", err);
                } else {
                    self.is_online = true;
                    let _ = self.send_dist(PayloadDist::Online).await;
                    break;
                }
                let poll_result = self.eventloop.poll().await;
                self.serve_poll(poll_result).await;
            }
            loop {
                if !self.is_active { break 'outer; }
                let poll_result = self.eventloop.poll().await;
                self.serve_poll(poll_result).await;
                if !self.is_online { break; }
                
            }
        }
        self.destruct().await;
    }

    async fn serve_poll(&mut self, poll_result: Result<Event, ConnectionError>) {
        match poll_result {
            Ok(notification) => self.serve_notification(notification).await,
            Err(err) => self.serve_error(err).await,
        }
    }

    async fn serve_notification(&mut self, notification: Event) {
        if let Event::Incoming(incoming) = notification {
            match incoming {
                Incoming::Publish(msg) => {
                    if !self.is_online {
                        self.is_online = true;
                        let _ = self.send_dist(PayloadDist::Online).await;
                    }
                    let _ = self.send_dist(PayloadDist::Data{topic: msg.topic, message: msg.payload}).await;
                },
                Incoming::Disconnect => if self.is_online {
                    self.is_online = false;
                    let _ = self.send_dist(PayloadDist::Offline).await;
                }
                _ => if !self.is_online {
                    self.is_online = true;
                    let _ = self.send_dist(PayloadDist::Online).await;
                }
            }
        }
    }

    async fn serve_error(&mut self, err: ConnectionError) {
        if self.is_online {
            self.is_online = false;
            let _ = self.send_dist(PayloadDist::Offline).await;
        }
        println!("[ERR] sub {:?}", err); // TODO: LOG ?
    }
}

#[derive(Debug)]
pub enum SubError {
    TopicsMissing,
    TopicsSubscribe(ClientError),
    QosMapping(MqttBytesError),
}
impl fmt::Display for SubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubError::TopicsMissing => write!(f, "topics not found for one of clients - probably Sub::serve().await called second time"),
            SubError::TopicsSubscribe(err) => write!(f, "topics not found for one of clients: {}", err),
            SubError::QosMapping(err) => write!(f, "qos mapping error: {}", err),
        }
    }
}
