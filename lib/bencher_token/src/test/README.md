Test keys for signing OAuth ID tokens. Never use them anywhere else.

Generate the signing key:

```
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out oauth_signing_key.pem
```

Generate the key that is too short to accept:

```
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:1024 -out oauth_signing_key_1024.pem
```
