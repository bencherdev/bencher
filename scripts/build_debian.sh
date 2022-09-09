HOST=$1

sudo apt update

sudo apt install git
git clone https://github.com/epompeii/bencher.git

curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

cd bencher
touch ./services/api/bencher.db
echo "VITE_BENCHER_API_URL=http://$HOST:8080" > ./services/ui/.env.development
sudo docker compose --file build.docker-compose.yml up --build --detach --remove-orphans
