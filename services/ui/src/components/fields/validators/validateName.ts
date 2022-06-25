function validateName(name) {
    if (name == null || name.trim().length < 4 || name.trim().length > 64) {
        return false;
    };

    // eslint-disable-next-line
    var re = /^[a-zA-Z\d\s\,\.\?\!\@\&\-\+\|\:\']{4,64}$/gm;
    return re.test(name);
};

export default validateName;