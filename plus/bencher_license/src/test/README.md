Generate Private Key:

```
openssl ecparam -genkey -noout -name prime256v1 \
    | openssl pkcs8 -topk8 -nocrypt -out private.pem
```

Generate Public Key:

```
openssl ec -in private.pem -pubout -out public.pem
```

`foreign.pem` is a second private key, made the same way, whose public half no build trusts.
The tests sign with it to show that a license signed by another key does not verify.