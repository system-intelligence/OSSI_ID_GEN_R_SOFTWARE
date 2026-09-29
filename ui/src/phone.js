// Philippine phone numbers for the Contact Number field.
// Mobile: accepts 09XXXXXXXXX, 9XXXXXXXXX, 639XXXXXXXXX or +639XXXXXXXXX, with any spaces/dashes,
//         and prints them in the card's style: +63 9XX-XXX-XXXX (the card formatting itself is done in Rust).
// Telephone (landline): area code + number, shown as (02) 8XXX XXXX for Metro Manila or (0XX) XXX XXXX elsewhere.
// Loaded as a plain <script>; exposes window.OssiPhone.
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

    // Number without the country code or trunk 0: "+63 917..." / "0917..." -> "917..."
    function nationalDigits(value) {
        let digits = digitsOf(value);
        if (digits.startsWith('63')) digits = digits.slice(2);
        if (digits.startsWith('0')) digits = digits.slice(1);
        return digits;
    }

    function group(digits, sizes) {
        const parts = [];
        let pos = 0;
        for (const size of sizes) {
            if (pos >= digits.length) break;
            parts.push(digits.slice(pos, pos + size));
            pos += size;
        }
        return parts.join(' ');
    }

    // What the mobile box shows after the fixed +63: "917 123 4567"
    function formatMobileInput(value) {
        return group(nationalDigits(value).slice(0, 10), [3, 3, 4]);
    }

    // Landline: Metro Manila is 2 + 8 digits, provincial numbers are a 2-digit area code + 7 digits
    function formatLandlineInput(value) {
        const digits = nationalDigits(value).slice(0, 9);
        if (!digits) return digitsOf(value).startsWith('0') ? '(0' : ''; // echo the first 0 of the area code
        const isManila = digits[0] === '2';
        const areaLength = isManila ? 1 : 2;
        if (digits.length <= areaLength) return `(0${digits}`;
        const area = digits.slice(0, areaLength);
        const rest = digits.slice(areaLength);
        return `(0${area}) ${group(rest, isManila ? [4, 4] : [3, 4])}`;
    }

    function isValidMobileInput(value) {
        return /^9\d{9}$/.test(nationalDigits(value));
    }

    function isValidLandlineInput(value) {
        return /^[2-8]\d{8}$/.test(nationalDigits(value));
    }

    window.OssiPhone = {
        looksLikeMobile,
        isValidMobile,
        formatContactNumber,
        formatMobileInput,
        formatLandlineInput,
        isValidMobileInput,
        isValidLandlineInput
    };
})();
