# What is this?

This is a project to try to detect any AWS access keys that are accidentally uploaded to the Python Package Index (PyPi).

New uploads are scanned periodically, and if a valid key is detected then it is added to this repository under the 
[keys directory](./keys/). This will then notify AWS (via 
[Github secret scanning](https://docs.github.com/en/code-security/secret-scanning/about-secret-scanning)) which will 
cause AWS to secure your key.

## What's wrong with adding IAM credentials into code?

It can lead to anyone using these to perform potentially malicious actions on your account. See the 
[AWS best practices document](https://docs.aws.amazon.com/accounts/latest/reference/credentials-access-keys-best-practices.html) 
for more details.

## How does it work?

This is a proof-of-concept that uses github actions to run a rust tool every hour. The [main.rs file](./src/main.rs) has 
a pretty good overview of the process and how it all works.

## What happens when my key is added?

AWS will notify you via an email and apply the [QuarantineV2 IAM policy](https://github.com/z0ph/MAMIP/blob/master/policies/AWSCompromisedKeyQuarantineV2) 
onto the leaked key.