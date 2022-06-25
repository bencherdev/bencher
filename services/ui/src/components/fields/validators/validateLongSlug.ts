function validateLongSlug(slug) {
    if (slug == null || slug.trim().length < 4 || slug.trim().length > 128 || Number.isInteger(+slug)) {
        return false;
    };

    // eslint-disable-next-line
    var re = /^[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*$/gm;
    return re.test(slug);
};

export default validateLongSlug;