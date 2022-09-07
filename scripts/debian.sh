sudo apt update

sudo apt install git
git clone https://github.com/epompeii/bencher.git

curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

cd bencher
touch ./services/api/bencher.db
sudo docker compose up --build --detach --remove-orphans
