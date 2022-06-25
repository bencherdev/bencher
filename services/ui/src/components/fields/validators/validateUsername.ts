function validateUsername(username) {
    if (username == null || username.trim().length < 5 || username.trim().length > 32) {
        return false;
    };

    // eslint-disable-next-line
    var re = /^[a-zA-Z\d\s\,\.\-\']{5,32}$/gm;
    return re.test(username);
};

export default validateUsername;