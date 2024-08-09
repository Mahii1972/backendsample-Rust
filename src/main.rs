//page to get request from user and send response to user and send the no of connections to nats server
//imports
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_nats::ConnectOptions;
use tokio::sync::Semaphore;
use std::collections::HashSet;
use rand::Rng;

const HOST: &str = "127.0.0.1";
const PORTS: [u16; 3] = [8081, 8082, 8083];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connections = Arc::new(AtomicUsize::new(0));
    let active_ports = Arc::new(tokio::sync::Mutex::new(HashSet::new()));
    let active_connections = Arc::new([
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ]);

    let nats_client = Arc::new(ConnectOptions::new().connect("nats://localhost:4222").await?);
    let semaphore = Arc::new(Semaphore::new(50));

    // Spawn a thread to manage active ports
    let active_ports_clone = Arc::clone(&active_ports);
    tokio::spawn(async move {
        loop {
            {
                let mut ports = active_ports_clone.lock().await;
                ports.extend(PORTS);
                println!("All ports active: {:?}", ports);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

            for &port_to_disable in &PORTS {
                {
                    let mut ports = active_ports_clone.lock().await;
                    ports.remove(&port_to_disable);
                    println!("Disabled port {}. Active ports: {:?}", port_to_disable, ports);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;

                {
                    let mut ports = active_ports_clone.lock().await;
                    ports.insert(port_to_disable);
                    println!("Re-enabled port {}. Active ports: {:?}", port_to_disable, ports);
                }
            }
        }
    });

    // Create listeners and spawn tasks for each port
    let mut listener_handles = vec![];
    for (index, &port) in PORTS.iter().enumerate() {
        let listener = TcpListener::bind((HOST, port)).await?;
        println!("Listening on port {}", port);

        let active_ports = Arc::clone(&active_ports);
        let semaphore = Arc::clone(&semaphore);
        let connections = Arc::clone(&connections);
        let nats_client = Arc::clone(&nats_client);
        let active_connections = Arc::clone(&active_connections);

        let handle = tokio::spawn(async move {
            loop {
                if active_ports.lock().await.contains(&port) {
                    if let Ok((stream, addr)) = listener.accept().await {
                        let semaphore = Arc::clone(&semaphore);
                        let connections = Arc::clone(&connections);
                        let nats_client = Arc::clone(&nats_client);
                        let active_connections = Arc::clone(&active_connections);
                        tokio::spawn(async move {
                            handle_connection(stream, addr, port, index, semaphore, connections, nats_client, active_connections).await;
                        });
                    }
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        });

        listener_handles.push(handle);
    }

    // Wait for all listener tasks to complete (which they never will)
    futures::future::join_all(listener_handles).await;

    Ok(())
}

async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    port: u16,
    port_index: usize,
    semaphore: Arc<Semaphore>,
    connections: Arc<AtomicUsize>,
    nats_client: Arc<async_nats::Client>,
    active_connections: Arc<[AtomicUsize; 3]>,
) {
    let _permit = semaphore.acquire().await.unwrap();
    let new_count = connections.fetch_add(1, Ordering::SeqCst) + 1;
    let _port_count = active_connections[port_index].fetch_add(1, Ordering::SeqCst) + 1;
    println!("New connection from {} on port {}. Total connections: {}", addr, port, new_count);
    
    // Get counts for all ports
    let counts: Vec<usize> = active_connections.iter()
        .map(|counter| counter.load(Ordering::SeqCst))
        .collect();
    
    // Publish updated active connection count to NATS
    let nats_message = format!("Current active connections: {}:{}:{}", 
        counts[0], counts[1], counts[2]);
    if let Err(e) = nats_client.publish("active_connections", nats_message.into()).await {
        eprintln!("Failed to publish to NATS: {}", e);
    }
    
    // Handle the connection here
    // For example, echo back any received data with a random delay
    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                // Random delay between 1 and 25 seconds
                let delay = rand::thread_rng().gen_range(1..=25);
                println!("Connection from {} on port {}: Starting delay of {} seconds", addr, port, delay);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                println!("Connection from {} on port {}: Finished delay of {} seconds", addr, port, delay);
                
                if let Err(e) = stream.write_all(&buffer[..n]).await {
                    eprintln!("Failed to write to stream: {}", e);
                    break;
                }
            },
            Err(e) => {
                eprintln!("Failed to read from stream: {}", e);
                break;
            }
        }
    }
    
    // Decrement the active connection count for this port
    active_connections[port_index].fetch_sub(1, Ordering::SeqCst);
    
    // Get updated counts for all ports
    let updated_counts: Vec<usize> = active_connections.iter()
        .map(|counter| counter.load(Ordering::SeqCst))
        .collect();
    
    // Publish updated active connection count to NATS
    let updated_nats_message = format!("Current active connections: {}:{}:{}", 
        updated_counts[0], updated_counts[1], updated_counts[2]);
    if let Err(e) = nats_client.publish("active_connections", updated_nats_message.into()).await {
        eprintln!("Failed to publish to NATS: {}", e);
    }
    
    // When done, the permit will be automatically released
}