# minecraft_proxyd
VERY EARLY ALPHA

## Working theory
- When a user connects, we run a health check on the minecraft server.
- If it's not up, we start the server, do continous health checks until the server is up and then pass through a TCP stream from the server back to the client.
- If it's up, we simply pass through the connection.
- We also run periodic checks to get the player count, if there are no players online for 30 minutes, we shut down the server.