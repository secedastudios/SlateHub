-- ------------------------------
-- ACCESS DEFINITIONS (for authentication)
-- ------------------------------

-- User access for signup/signin
DEFINE ACCESS user ON DATABASE TYPE RECORD
    SIGNUP ( CREATE person SET username = $user, email = $email, password = crypto::argon2::generate($pass) )
    SIGNIN ( SELECT * FROM person WHERE username = $user AND crypto::argon2::compare(password, $pass) )
    DURATION FOR TOKEN 15m, FOR SESSION 12h;
