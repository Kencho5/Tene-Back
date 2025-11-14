aws ecr get-login-password --region eu-central-1 --profile tene | docker login --username AWS --password-stdin 037721735321.dkr.ecr.eu-central-1.amazonaws.com
docker build -t tene-back .
docker tag tene-back:latest 037721735321.dkr.ecr.eu-central-1.amazonaws.com/tene-back:latest
docker push 037721735321.dkr.ecr.eu-central-1.amazonaws.com/tene-back:latest
