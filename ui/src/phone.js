// Philippine mobile numbers: accepts 09XXXXXXXXX, 9XXXXXXXXX, 639XXXXXXXXX or +639XXXXXXXXX,
// with any spaces/dashes, and prints them in the card's style: +63 9XX-XXX-XXXX.
// Loaded as a plain <script>; exposes window.OssiPhone (the card formatting itself is done in Rust).
(function () {
    const PH_MOBILE = /^(?:0|63)?(9\d{2})(\d{3})(\d{4})$/;

    function digitsOf(value) {
        return String(value || '').replace(/\D/g, '');
    }

    // Typed like a mobile number (starts with 09, 9, 63 or +63) - these must be valid to print
    function looksLikeMobile(value) {
        return /^(?:09|9|639)/.test(digitsOf(value));
    }

    function isValidMobile(value) {
        return PH_MOBILE.test(digitsOf(value));
    }

    // Mobile numbers get the card format; anything else (e.g. landlines) is printed as typed
    function formatContactNumber(value) {
        const match = digitsOf(value).match(PH_MOBILE);
        if (!match) return String(value || '').trim();
        return `+63 ${match[1]}-${match[2]}-${match[3]}`;
    }

    window.OssiPhone = { looksLikeMobile, isValidMobile, formatContactNumber };
})();
