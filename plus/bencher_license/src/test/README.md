Generate Private Key:

```
openssl ecparam -genkey -noout -name prime256v1 \
    | openssl pkcs8 -topk8 -nocrypt -out private.pem
```

Generate Public Key:

```
openssl ec -in private.pem -pubout -out public.pem
```