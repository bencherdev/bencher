function validateDescription(description) {
    if (description == null || description.trim().length < 25 || description.trim().length > 2500) {
        return false;
    };

    // eslint-disable-next-line
    var re = /^(.|\s)*[a-zA-Z]+(.|\s)*$/gm;
    return re.test(description);
};

export default validateDescription;