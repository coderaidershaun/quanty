# Keep the stores on a server

quanty can keep its library on a server instead of on your own machine. Then every machine you work on, and every agent, uses the same library. This page sets that up for a Linux server that runs Docker and that you reach with SSH.

No quanty program runs on the server. The server runs the two stores, Qdrant and FalkorDB, in Docker, and it shares the folder with the converted pages. quanty on each of your machines talks to the stores through an SSH tunnel.

```
your machine                                  the server
quanty ── 127.0.0.1:16334 ─┐                  127.0.0.1:6334  Qdrant  (API key)
          127.0.0.1:16379 ─┼── SSH tunnel ──► 127.0.0.1:6379  FalkorDB (password)
          127.0.0.1:10445 ─┘                  127.0.0.1:445   the content share (optional)
```

Nothing on the server listens to the internet except SSH. The two stores still have a key and a password, because a port on the server's own loopback address can be reached by every account on that server.

[Choose a database](guide.md#choose-a-database) in the guide describes the sheet in the app that you use at the end. [Where a library lives](guide.md#where-a-library-lives) describes the three folders that make a library.

## 1. Start the stores on the server

Log in to the server and make a folder for the library. `/srv/quanty` is the example. It is the root of the library: the stores keep their files under `data/` in it, and the converted pages go in `content/`.

```bash
sudo mkdir -p /srv/quanty && sudo chown "$USER" /srv/quanty
cd /srv/quanty
mkdir content
```

Make the key and the password. These two commands write them to a file that only you can read, and print nothing. Keep a copy in your password manager, because the app asks for both.

```bash
printf 'QDRANT_API_KEY=%s\nFALKORDB_PASSWORD=%s\n' "$(openssl rand -hex 32)" "$(openssl rand -hex 32)" > .env
chmod 600 .env
```

This `.env` belongs to the server's Docker Compose. It is not the `.env` of quanty. Read the values with `cat .env` when you need them.

Write the Compose file. It is the `docker-compose.yaml` of the quanty repository, changed in a few places. Both ports bind to the loopback address of the server (`127.0.0.1`), and the ports that quanty does not use are gone: 6333, the REST port of Qdrant, and 3000, the browser of FalkorDB. Qdrant gets its API key. FalkorDB gets its password.

```bash
cat > docker-compose.yaml <<'EOF'
services:
  qdrant:
    image: qdrant/qdrant
    ports: ["127.0.0.1:6334:6334"]
    environment:
      - QDRANT__SERVICE__API_KEY=${QDRANT_API_KEY}
    volumes: ["./data/qdrant:/qdrant/storage"]
  falkordb:
    image: falkordb/falkordb
    ports: ["127.0.0.1:6379:6379"]
    environment:
      - REDIS_ARGS=--requirepass ${FALKORDB_PASSWORD}
      - REDISCLI_AUTH=${FALKORDB_PASSWORD}
    volumes: ["./data/falkordb:/var/lib/falkordb/data"]
EOF
```

The settings come from the makers of the stores:

- **Qdrant** takes its API key from `QDRANT__SERVICE__API_KEY`, and a client sends it in the `api-key` header. See [Qdrant security](https://qdrant.tech/documentation/security/). Qdrant says that a key sent over a connection that is not encrypted can leak, which is why the tunnel matters: do not open port 6334 to the internet.
- **FalkorDB** takes a password through `REDIS_ARGS`, as `--requirepass`. See the [FalkorDB configuration page](https://docs.falkordb.com/getting-started/configuration.html).
- **`REDISCLI_AUTH`** is read by `redis-cli`, the tool inside the FalkorDB container. With it set there, the backup commands below need no password on their command line, where a process list and a shell history would show it. See the [`redis-cli` page](https://redis.io/docs/latest/develop/tools/cli/).

Start the stores and check them:

```bash
docker compose up -d
docker compose ps                              # both say running
docker compose exec falkordb redis-cli PING    # PONG: the password works
```

Anyone who can run Docker on the server can read the key and the password, so give that right only to people who may read the library.

## 2. Open the tunnel

Do this on each machine that uses the library, before you start quanty. Replace `me@server` with your SSH login.

```bash
ssh -N -L 16334:127.0.0.1:6334 -L 16379:127.0.0.1:6379 -L 10445:127.0.0.1:445 me@server
```

It prints nothing and stays open, so give it a terminal of its own. Each `-L` brings one port of the server to a port of your machine:

| On your machine | Goes to, on the server | What it is |
| --- | --- | --- |
| `127.0.0.1:16334` | `127.0.0.1:6334` | Qdrant |
| `127.0.0.1:16379` | `127.0.0.1:6379` | FalkorDB |
| `127.0.0.1:10445` | `127.0.0.1:445` | The content share, only if you use SMB in step 3 |

The local ports are not 6334 and 6379, so the Docker stores on your own machine, Local, and the server can both be used on the same day. Leave the last `-L` out if you use `sshfs` in step 3. To make a dropped connection end the command, so that you notice, add `-o ExitOnForwardFailure=yes -o ServerAliveInterval=30`.

Check that both ports answer:

```bash
nc -z 127.0.0.1 16334 && nc -z 127.0.0.1 16379 && echo "the tunnel is open"
```

## 3. Share the content folder

The converted pages of every document are files under the content folder. The Library, the Source panel and `read_page` read them, and an ingest writes them. So every machine must reach the same folder, the one on the server. Choose one way. Both mount the folder on your machine.

**Mount it at the same path on every machine, for example `/Volumes/quanty-content`, and mount it before you start quanty.** There are two reasons:

- The pictures of figures in an answer are looked for at the path they were saved under. They show only when every machine reaches the folder at the same path.
- quanty refuses a saved database whose content folder is not there. If it were not mounted and quanty made the folder itself, an ingest would write its pages to your own disk.

### With `sshfs`

Nothing to install on the server. On your machine you need `sshfs`, and on a Mac also macFUSE (see their pages); on Debian or Ubuntu, `sudo apt install sshfs`. It uses SSH itself, so it does not need the tunnel's last `-L`.

```bash
sudo mkdir -p /Volumes/quanty-content && sudo chown "$USER" /Volumes/quanty-content
sshfs me@server:/srv/quanty/content /Volumes/quanty-content -o reconnect
```

Unmount it with `umount /Volumes/quanty-content`, or on Linux `fusermount -u /Volumes/quanty-content`.

### With SMB through the tunnel

SMB is the file sharing that a Mac and a Windows PC have built in. Samba on the server answers on its loopback address only, and the tunnel's last `-L` brings it to port 10445 of your machine.

On the server, install Samba, give your account a Samba password, and make it listen on the loopback address only:

```bash
sudo apt install samba
sudo smbpasswd -a "$USER"
```

In `/etc/samba/smb.conf`, add these two lines inside the existing `[global]` section, and the share at the end of the file. Replace `me` with your account.

```ini
   interfaces = lo
   bind interfaces only = yes
```

```ini
[quanty-content]
   path = /srv/quanty/content
   valid users = me
   read only = no
```

Then restart it with `sudo systemctl restart smbd`.

On a Mac, with the tunnel open, choose Go, then Connect to Server in the Finder (`⌘K`) and type `smb://127.0.0.1:10445/quanty-content`. The Mac mounts it at `/Volumes/quanty-content`. On Linux, with `cifs-utils` installed:

```bash
sudo mkdir -p /Volumes/quanty-content
sudo mount -t cifs //127.0.0.1/quanty-content /Volumes/quanty-content -o port=10445,username=me,uid=$(id -u),gid=$(id -g)
```

If the tunnel drops, a mounted folder stops answering. Open the tunnel again, then unmount and mount the folder again.

Check the mount before you start quanty:

```bash
ls /Volumes/quanty-content
```

## 4. Add the database in the app

With the tunnel open and the folder mounted, start quanty. Click the entry at the right end of the footer (**Database: …**), choose **New…** in the **Database** list, fill in the seven fields, and press **Use**.

| Field | What to put in it |
| --- | --- |
| **Name** | Any name, such as `Server` |
| **Qdrant URL** | `http://127.0.0.1:16334` |
| **Qdrant API key** | `QDRANT_API_KEY` from the server's `.env` |
| **FalkorDB URL** | `falkor://127.0.0.1:16379` |
| **FalkorDB password** | `FALKORDB_PASSWORD` from the server's `.env` |
| **Graph** | `quanty` |
| **Content folder** | The mount, `/Volumes/quanty-content` |

**Use** connects to both stores and checks the content folder, and switches only when all three are fine. If it cannot, the sheet says what, and the database you had stays. The footer then reads **Database: Server · connected**. The choice is saved in `.quanty-databases.json`, so the next start uses it, and so do `rag-ingest`, `rag-query`, `rag-dataset` and the MCP server of that quanty folder.

Check from the command line:

```bash
cargo run --release -p rag-ingestion --bin rag-ingest -- health
```

The Qdrant and FalkorDB lines name the loopback addresses of the tunnel and say `ok`. A `FAILED` line says what is wrong. The usual causes are a tunnel that is not open and a key or password that was copied wrong, and the [Troubleshooting](troubleshooting.md) table lists them.

The MCP server reads the choice only when it starts, so restart it after a switch. If you give an MCP client one of the store settings, the server ignores the chosen database: [mcp.md](mcp.md#which-database-it-uses) says why and how.

## 5. Back the library up

The library is the three folders under the root: `content`, `data/qdrant` and `data/falkordb`. A backup is a copy of them taken while the stores are stopped. Run this on the server, in `/srv/quanty`, with no ingest running on any machine.

```bash
cd /srv/quanty
docker compose exec falkordb redis-cli SAVE
docker compose stop
BACKUP="$HOME/quanty-library-$(date +%F).tar.gz"
sudo tar -czf "$BACKUP" content data/qdrant data/falkordb
sudo chown "$USER" "$BACKUP"
docker compose start
```

- `SAVE` makes FalkorDB write its whole graph to its file in `data/falkordb` now, before it stops. FalkorDB's own file is the copy that matters. Its documentation says that `SAVE` blocks until the snapshot is written, and Redis says that a snapshot file is never changed once it is made.
- The password is not on any command line. `redis-cli` reads it from `REDISCLI_AUTH`, which step 1 set in the container. Do not use `redis-cli -a`: the password would show in the process list and in your shell history.
- The `sudo` is because the stores' files belong to the account their container runs as, often root.
- Qdrant's own backup tool is the snapshot. A copy of its stopped folder is simpler and keeps the library in one place. After you restore one, check it with `rag-ingest health` and one search.

Copy the file off the server, so that a lost disk loses nothing:

```bash
scp me@server:quanty-library-2026-10-22.tar.gz .
```

To load a backup on your own machine, use the steps in [Where a library lives](guide.md#where-a-library-lives).

## 6. Move a library

This is the same for a library that comes from another server, from a backup, or from the Docker stores of your laptop. A laptop keeps its library in the same three folders under the quanty folder, so a laptop's library moves to a server in the same way.

1. **Make the tarball** where the library is now. On a server, step 5 does it. On a laptop, the [back up](guide.md#where-a-library-lives) commands of the guide do it. A tarball you already have is the same.
2. **Set the new server up** with step 1 above, up to the Compose file, but do not run `docker compose up -d` yet. New keys are fine: a key and a password are settings of the stores and are not in the library.
3. **Put the library in** on the new server, then start the stores:

   ```bash
   scp quanty-library-2026-10-22.tar.gz me@newserver:
   ssh me@newserver
   cd /srv/quanty
   sudo tar -xzf ~/quanty-library-2026-10-22.tar.gz
   docker compose up -d
   ```

4. **Share the content folder** again as in step 3, and mount it on your machine at the same path as before.
5. **Open the tunnel** to the new server and **add the database** in the app, as in step 4, with the new keys. Or change the address and the keys of the database that you had. Press **Use**.

The Library then lists the moved documents.

- **Figure pictures.** The pictures of figures in an answer are stored under the old absolute path of the content folder. A moved library shows them only when its content folder is reached at the same path as before. Pages, formulas, tables and code do not depend on it.
- **The Qdrant version.** Run the same Qdrant on the new server, or a newer one by at most one minor version. Qdrant states this rule for restoring its snapshots, and it is the safe rule for a copied folder too. The version is at the top of the log: `docker compose logs qdrant | head`.
- **The kept answers** (`data/concept-cache`, `data/dataset-cache`) and the built datasets are not part of the library, and the tarball does not hold them.

## 7. A private network instead of a tunnel

A private network such as Tailscale or WireGuard works the same way. Install it on the server and on your machines, bind the two ports to the server's private address in place of `127.0.0.1` in step 1, and use that address in place of `127.0.0.1:16334` and `127.0.0.1:16379` in the app. There is no tunnel command to run.
