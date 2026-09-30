// Tauri bridge: keeps the Electron-style ipcRenderer.invoke(channel, ...args) calls below unchanged
// and forwards them to the Rust commands in src-tauri/src/main.rs.
const ipcRenderer = {
    invoke(channel, ...args) {
        const { invoke } = window.__TAURI__.core;
        switch (channel) {
            case 'check-pin-setup': return invoke('check_pin_setup');
            case 'setup-pin': return invoke('setup_pin', { pin: args[0] });
            case 'verify-pin': return invoke('verify_pin', { pin: args[0] });
            case 'get-cities': return invoke('get_cities');
            case 'get-city-code': return invoke('get_city_code', { cityName: args[0] ?? null });
            case 'generate-control-number': return invoke('generate_control_number', { prefix: args[0], isRehire: !!args[1] });
            case 'get-all-records': return invoke('get_all_records');
            case 'reset-database': return invoke('reset_database');
            case 'generate-svg': return invoke('generate_svg', { data: args[0] });
            case 'get-card-preview': return invoke('get_card_preview', { controlNumber: args[0] });
            case 'open-card-folder': return invoke('open_card_folder', { controlNumber: args[0] });
            case 'get-printer-status': return invoke('get_printer_status');
            case 'log-print': return invoke('log_print', { controlNumber: args[0], side: args[1], reason: args[2] ?? null, printer: args[3] ?? null });
            case 'get-print-history': return invoke('get_print_history', { controlNumber: args[0] });
            case 'get-print-summary': return invoke('get_print_summary');
            case 'get-print-log': return invoke('get_print_log');
            default: return Promise.reject(new Error(`Unknown channel: ${channel}`));
        }
    }
};
const { formatMobileInput, formatLandlineInput, isValidMobileInput, isValidLandlineInput } = window.OssiPhone;

const idPicInput = document.getElementById('idPicInput');
const sigInput = document.getElementById('sigInput');
const preview = document.getElementById('preview');
const sigPreview = document.getElementById('sigPreview');
const downloadBtn = document.getElementById('downloadBtn');
const updateBtn = document.getElementById('updateBtn');
const status = document.getElementById('status');

const lastNameInput = document.getElementById('lastName');
const firstNameInput = document.getElementById('firstName');
const positionInput = document.getElementById('position');
const positionOtherInput = document.getElementById('position-other');

const middleInitialInput = document.getElementById('middleInitial');
const suffixInput = document.getElementById('suffix');

const idDropArea = document.getElementById('idDropArea');
const sigDropArea = document.getElementById('sigDropArea');

const idPlaceholder = document.querySelector('#idDropArea').parentElement.querySelector('.placeholder-text');
const sigPlaceholder = document.querySelector('#sigDropArea').parentElement.querySelector('.placeholder-text');

const backFirstNameInput = document.getElementById('back-firstName');
const backMiddleInitialInput = document.getElementById('back-middleInitial');
const backSurnameInput = document.getElementById('back-surname');
const backSuffixInput = document.getElementById('back-suffix');
const backAddress1Input = document.getElementById('back-address1');
const backAddress2Input = document.getElementById('back-address2');
const backRelationshipInput = document.getElementById('back-relationship');
const backContactInput = document.getElementById('back-contact');
const backHireYear = document.getElementById('back-hire-year');
const backHireMonth = document.getElementById('back-hire-month');
const backHireDay = document.getElementById('back-hire-day');
const citySelect = document.getElementById('city-select');
const cityOptions = document.getElementById('city-options');
const cityTrigger = document.getElementById('city-trigger');
const cityWrapper = document.getElementById('city-wrapper');
const backRehire = document.getElementById('back-rehire');

let idPicBase64 = null;
let sigBase64 = null;
let dataUpdated = false;
let activeTab = 'front';
let generatedControlNumber = '';
// Control number issued by the last front download; the back ID is filed under it
let issuedControlNumber = '';
// SVG (data URL) of the card last downloaded from each tab, cleared as soon as that tab is edited again
const lastGenerated = { front: null, back: null };
let recordsSearchText = '';
let recordsSortColumn = 'createdAt';
let recordsSortDirection = 'desc';


// Custom Select Functionality
document.addEventListener('DOMContentLoaded', function() {
    // PIN Lock Screen Logic
    const lockScreen = document.getElementById('lockScreen');
    const lockSubtitle = document.getElementById('lockSubtitle');
    const pinInputsContainer = document.getElementById('pinInputs');
    const unlockBtn = document.getElementById('unlockBtn');
    const unlockBtnText = document.getElementById('unlockBtnText');
    const lockError = document.getElementById('lockError');
    const pinBoxes = pinInputsContainer.querySelectorAll('.pin-box');
    const container = document.querySelector('.container');

    let isUnlocked = false;
    let isSetupMode = false;

    function hideMainApp() {
        if (container) container.style.display = 'none';
    }

    function showMainApp() {
        if (container) container.style.display = '';
    }

    function showError() {
        pinBoxes.forEach(box => box.classList.add('error'));
        lockError.textContent = 'Incorrect PIN. Please try again.';
        setTimeout(() => {
            pinBoxes.forEach(box => box.classList.remove('error'));
            pinBoxes.forEach(box => box.value = '');
            pinBoxes[0].focus();
        }, 600);
    }

    async function checkPinSetup() {
        const result = await ipcRenderer.invoke('check-pin-setup');
        isSetupMode = !result.setupComplete;
        if (isSetupMode) {
            lockSubtitle.textContent = 'Create a 4-digit PIN to secure the app';
            unlockBtnText.textContent = 'Set PIN';
        } else {
            lockSubtitle.textContent = 'Enter PIN to continue';
            unlockBtnText.textContent = 'Unlock';
        }
        pinBoxes[0].focus();
    }

    async function handleUnlock() {
        const pin = Array.from(pinBoxes).map(box => box.value).join('');
        if (pin.length !== 4) {
            lockError.textContent = 'Please enter a 4-digit PIN';
            return;
        }

        if (isSetupMode) {
            const result = await ipcRenderer.invoke('setup-pin', pin);
            if (result.success) {
                isUnlocked = true;
                lockScreen.style.display = 'none';
                showMainApp();
            } else {
                lockError.textContent = 'Failed to setup PIN. Please try again.';
                pinBoxes.forEach(box => { box.value = ''; box.classList.remove('error'); });
                pinBoxes[0].focus();
            }
            return;
        }

        const result = await ipcRenderer.invoke('verify-pin', pin);
        if (result.success) {
            isUnlocked = true;
            lockScreen.style.display = 'none';
            showMainApp();
        } else {
            showError();
        }
    }

    pinBoxes.forEach((box, index) => {
        box.addEventListener('input', function(e) {
            box.value = box.value.replace(/\D/g, ''); // PIN is digits only
            if (box.value.length === 1 && index < pinBoxes.length - 1) {
                pinBoxes[index + 1].focus();
            }
            lockError.textContent = '';
        });

        box.addEventListener('keydown', function(e) {
            if (e.key === 'Backspace' && box.value === '' && index > 0) {
                pinBoxes[index - 1].focus();
            }
            if (e.key === 'Enter') {
                handleUnlock();
            }
        });

        box.addEventListener('paste', function(e) {
            e.preventDefault();
            const paste = (e.clipboardData || window.clipboardData).getData('text').slice(0, 4);
            const digits = paste.replace(/\D/g, '').split('');
            digits.forEach((digit, i) => {
                if (pinBoxes[i]) pinBoxes[i].value = digit;
            });
            if (digits.length > 0 && digits.length < 4) {
                pinBoxes[digits.length].focus();
            } else if (digits.length === 4) {
                handleUnlock();
            }
        });
    });

    unlockBtn.addEventListener('click', handleUnlock);

    hideMainApp();
    checkPinSetup();

    // Front suffix custom select
    const suffixWrapper = document.getElementById('suffix-wrapper');
    const suffixTrigger = document.getElementById('suffix-trigger');
    const suffixSelect = document.getElementById('suffix');
    const suffixOptions = suffixWrapper.querySelectorAll('.option');
    const suffixChevron = suffixWrapper.querySelector('.chevron');
    
    suffixTrigger.addEventListener('click', function(e) {
        e.stopPropagation();
        suffixWrapper.classList.toggle('open');
        suffixChevron.classList.toggle('rotate');
    });
    
    suffixOptions.forEach(option => {
        option.addEventListener('click', function() {
            const value = this.dataset.value;
            suffixSelect.value = value;
            suffixTrigger.querySelector('.select-text').textContent = this.textContent;
            suffixWrapper.classList.remove('open');
            suffixChevron.classList.remove('rotate');
            
            // Trigger change event
            const event = new Event('change', { bubbles: true });
            suffixSelect.dispatchEvent(event);
        });
    });
    
    // Back suffix custom select
    const backSuffixWrapper = document.getElementById('back-suffix-wrapper');
    const backSuffixTrigger = document.getElementById('back-suffix-trigger');
    const backSuffixSelect = document.getElementById('back-suffix');
    const backSuffixOptions = backSuffixWrapper.querySelectorAll('.option');
    const backSuffixChevron = backSuffixWrapper.querySelector('.chevron');
    
    backSuffixTrigger.addEventListener('click', function(e) {
        e.stopPropagation();
        backSuffixWrapper.classList.toggle('open');
        backSuffixChevron.classList.toggle('rotate');
    });
    
    backSuffixOptions.forEach(option => {
        option.addEventListener('click', function() {
            const value = this.dataset.value;
            backSuffixSelect.value = value;
            backSuffixTrigger.querySelector('.select-text').textContent = this.textContent;
            backSuffixWrapper.classList.remove('open');
            backSuffixChevron.classList.remove('rotate');
            
            // Trigger change event
            const event = new Event('change', { bubbles: true });
            backSuffixSelect.dispatchEvent(event);
        });
    });
    
    // Relationship custom select
    const backRelationshipWrapper = document.getElementById('back-relationship-wrapper');
    const backRelationshipTrigger = document.getElementById('back-relationship-trigger');
    const backRelationshipOptions = backRelationshipWrapper.querySelectorAll('.option');
    const backRelationshipChevron = backRelationshipWrapper.querySelector('.chevron');

    backRelationshipTrigger.addEventListener('click', function(e) {
        e.stopPropagation();
        backRelationshipWrapper.classList.toggle('open');
        backRelationshipChevron.classList.toggle('rotate');
    });

    backRelationshipOptions.forEach(option => {
        option.addEventListener('click', function() {
            backRelationshipInput.value = this.dataset.value;
            backRelationshipTrigger.querySelector('.select-text').textContent = this.textContent;
            backRelationshipWrapper.classList.remove('open');
            backRelationshipChevron.classList.remove('rotate');

            const event = new Event('change', { bubbles: true });
            backRelationshipInput.dispatchEvent(event);
        });
    });

    // Position dropdown
    const positionWrapper = document.getElementById('position-wrapper');
    const positionTrigger = document.getElementById('position-trigger');
    const positionSelect = document.getElementById('position');
    const positionOptions = positionWrapper.querySelectorAll('.option');
    const positionChevron = positionWrapper.querySelector('.chevron');
    
    positionTrigger.addEventListener('click', function(e) {
        e.stopPropagation();
        positionWrapper.classList.toggle('open');
        positionChevron.classList.toggle('rotate');
    });
    
    positionOptions.forEach(option => {
        option.addEventListener('click', function() {
            const value = this.dataset.value;
            positionSelect.value = value;
            positionTrigger.querySelector('.select-text').textContent = this.textContent;
            positionWrapper.classList.remove('open');
            positionChevron.classList.remove('rotate');
            
            if (value === 'OTHERS') {
                positionOtherInput.style.display = 'block';
                positionOtherInput.focus();
            } else {
                positionOtherInput.style.display = 'none';
                positionOtherInput.value = '';
            }
            
            const event = new Event('change', { bubbles: true });
            positionSelect.dispatchEvent(event);
        });
    });
    
    // City custom select
    const cityWrapperLocal = document.getElementById('city-wrapper');
    const cityTriggerLocal = document.getElementById('city-trigger');
    const cityOptionsContainer = document.getElementById('city-options');
    const cityChevron = cityWrapperLocal.querySelector('.chevron');
    
    cityTriggerLocal.addEventListener('click', function(e) {
        e.stopPropagation();
        cityWrapperLocal.classList.toggle('open');
        cityChevron.classList.toggle('rotate');
    });
    
    cityOptionsContainer.querySelectorAll('.option').forEach(option => {
        option.addEventListener('click', function() {
            const value = this.dataset.value;
            citySelect.value = value;
            cityTriggerLocal.querySelector('.select-text').textContent = this.textContent;
            cityWrapperLocal.classList.remove('open');
            cityChevron.classList.remove('rotate');
            autoGenerateControlNumber();
        });
    });
    
    // Grey out placeholder values and highlight the chosen option in every custom select
    function syncSelectStates() {
        document.querySelectorAll('.select-wrapper').forEach(wrapper => {
            const select = wrapper.querySelector('select');
            wrapper.querySelector('.select-trigger').classList.toggle('is-placeholder', select.value === '');
            wrapper.querySelectorAll('.option').forEach(option => {
                option.classList.toggle('selected', option.dataset.value === select.value);
            });
        });
    }
    syncSelectStates();
    document.addEventListener('click', syncSelectStates);

    // Open custom selects from the keyboard
    document.addEventListener('keydown', function(e) {
        if (!e.target.classList.contains('select-trigger')) return;
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            e.target.click();
        } else if (e.key === 'Escape') {
            document.body.click();
        }
    });

    // Searchable dropdowns (City of Birth): type to filter, arrow keys to move, Enter to pick, Esc to close
    document.querySelectorAll('.select-options.searchable').forEach(panel => {
        const wrapper = panel.closest('.select-wrapper');
        const trigger = wrapper.querySelector('.select-trigger');
        const chevron = wrapper.querySelector('.chevron');
        const input = panel.querySelector('.select-search-input');
        const noResults = panel.querySelector('.select-no-results');

        const visibleOptions = () => Array.from(panel.querySelectorAll('.option')).filter(o => !o.hidden);

        function highlight(option) {
            panel.querySelectorAll('.option.highlighted').forEach(o => o.classList.remove('highlighted'));
            if (option) {
                option.classList.add('highlighted');
                option.scrollIntoView({ block: 'nearest' });
            }
        }

        // While searching, the "Select city" placeholder row is hidden and the first match is highlighted
        function filter() {
            const query = input.value.trim().toLowerCase();
            let count = 0;
            panel.querySelectorAll('.option').forEach(option => {
                const match = !query || (option.dataset.value !== '' && option.textContent.toLowerCase().includes(query));
                option.hidden = !match;
                if (match) count++;
            });
            noResults.hidden = count > 0;
            highlight(query ? visibleOptions()[0] : null);
        }

        function close() {
            wrapper.classList.remove('open');
            chevron.classList.remove('rotate');
            trigger.focus();
        }

        // Runs after the trigger's own toggle handler: on opening, start with an empty search
        trigger.addEventListener('click', () => {
            if (!wrapper.classList.contains('open')) return;
            input.value = '';
            filter();
            setTimeout(() => input.focus(), 0);
        });

        input.addEventListener('input', filter);
        // Clicking in the search box must not reach the document "click outside closes dropdowns" handler
        input.addEventListener('click', e => e.stopPropagation());

        input.addEventListener('keydown', e => {
            const options = visibleOptions();
            const current = options.indexOf(panel.querySelector('.option.highlighted'));
            if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
                e.preventDefault();
                if (!options.length) return;
                const next = e.key === 'ArrowDown' ? Math.min(current + 1, options.length - 1) : Math.max(current - 1, 0);
                highlight(options[next]);
            } else if (e.key === 'Enter') {
                e.preventDefault();
                const chosen = options[current] || (options.length === 1 ? options[0] : null);
                if (chosen) chosen.click();
            } else if (e.key === 'Escape') {
                e.preventDefault();
                close();
            } else if (e.key === 'Tab') {
                close();
            }
        });
    });

    // Close dropdowns when clicking outside
    document.addEventListener('click', function() {
        suffixWrapper.classList.remove('open');
        suffixChevron.classList.remove('rotate');
        backSuffixWrapper.classList.remove('open');
        backSuffixChevron.classList.remove('rotate');
        backRelationshipWrapper.classList.remove('open');
        backRelationshipChevron.classList.remove('rotate');
        positionWrapper.classList.remove('open');
        positionChevron.classList.remove('rotate');
        if (cityWrapperLocal) {
            cityWrapperLocal.classList.remove('open');
            if (cityChevron) cityChevron.classList.remove('rotate');
        }
    });
    
    // New ID button - reset all fields
    const newIdBtn = document.getElementById('newIdBtn');
    if (newIdBtn) {
        newIdBtn.addEventListener('click', () => {
            document.querySelectorAll('input[type="text"], input[type="file"]').forEach(input => {
                if (input.type === 'file') {
                    input.value = '';
                } else {
                    input.value = '';
                }
            });
            document.querySelectorAll('select').forEach(select => select.value = '');
            positionSelect.value = 'SECURITY GUARD';
            document.querySelectorAll('.select-wrapper').forEach(wrapper => {
                const select = wrapper.querySelector('select');
                wrapper.querySelector('.select-text').textContent = select.options[select.selectedIndex].textContent.trim();
            });
            if (backRehire) backRehire.checked = false;
            setPhoneType('mobile');
            [preview, sigPreview].forEach(img => img.style.display = 'none');
            document.querySelectorAll('.placeholder-text').forEach(el => el.style.display = 'block');
            positionOtherInput.style.display = 'none';
            
            idPicBase64 = null;
            sigBase64 = null;
            dataUpdated = false;
            generatedControlNumber = '';
            issuedControlNumber = '';
            lastGenerated.front = null;
            lastGenerated.back = null;

            document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
            document.querySelectorAll('.tab-panel').forEach(panel => panel.classList.remove('active'));
            document.querySelector('.tab-btn[data-tab="front"]').classList.add('active');
            document.getElementById('front-panel').classList.add('active');
            activeTab = 'front';
            
            checkReady();
            status.innerHTML = '<i class="fas fa-info-circle"></i> <span id="status-text">Ready for new ID - upload images for Front ID</span>';
            status.className = 'status info';
        });
    }
    
    // Load cities for the picker
    async function loadCities() {
        try {
            const cities = await ipcRenderer.invoke('get-cities');
            const container = document.getElementById('city-options');
            const select = document.getElementById('city-select');
            container.innerHTML = '<div class="option" data-value="">Select city</div>';
            select.innerHTML = '<option value="">Select city</option>';
            cities.forEach(c => {
                const div = document.createElement('div');
                div.className = 'option';
                div.dataset.value = c.name;
                div.textContent = c.name;
                container.appendChild(div);
                const option = document.createElement('option');
                option.value = c.name;
                option.textContent = c.name;
                select.appendChild(option);
            });
            container.querySelectorAll('.option').forEach(option => {
                option.addEventListener('click', function() {
                    const value = this.dataset.value;
                    citySelect.value = value;
                    cityTrigger.querySelector('.select-text').textContent = this.textContent;
                    cityWrapper.classList.remove('open');
                    cityChevron.classList.toggle('rotate', false);
                    autoGenerateControlNumber();
                });
            });
        } catch (err) {
            console.error('Failed to load cities:', err);
        }
    }
    
    function autoGenerateControlNumber() {
        if (!backHireYear || !backHireMonth || !backHireDay || !citySelect) return;
        const hireDate = getHireDate();
        const city = citySelect.value;
        if (hireDate && city) {
            const dateStr = hireDate.replace(/-/g, '').slice(2);
            ipcRenderer.invoke('get-city-code', city).then(code => {
                if (code) {
                    const prefix = `${dateStr}${code}`;
                    const seq = String(Math.floor(Math.random() * 10000)).padStart(4, '0');
                    const suffix = backRehire && backRehire.checked ? '-RH' : '';
                    generatedControlNumber = `${prefix}-${seq}${suffix}`;
                }
            });
        }
    }
    
    function validateBackDate() {
        if (!backHireYear || !backHireMonth || !backHireDay) return false;
        return getHireDate() !== '';
    }
    
    [backHireYear, backHireMonth, backHireDay].forEach(el => {
        if (!el) return;
        el.addEventListener('input', function(e) {
            let v = this.value.replace(/\D/g, '');
            if (v.length > 2) v = v.slice(0, 2);
            this.value = v;
            if (validateBackDate() && citySelect && citySelect.value) {
                autoGenerateControlNumber();
            }
        });
    });
    
    if (citySelect) {
        citySelect.addEventListener('change', () => {
            if (validateBackDate()) autoGenerateControlNumber();
        });
    }
    
    if (backRehire) {
        backRehire.addEventListener('change', () => {
            if (validateBackDate() && citySelect && citySelect.value) {
                autoGenerateControlNumber();
            }
        });
    }
    
    loadCities();
});


function updatePlaceholder(placeholder, hasImage) {
    if (hasImage) {
        placeholder.style.display = 'none';
    } else {
        placeholder.style.display = 'block';
    }
}


// Auto-uppercase all text inputs on front panel + auto-dot for middle initial
function setupAutoCapitalize() {
    const frontInputs = document.querySelectorAll('#front-panel input[type="text"]');
    frontInputs.forEach(input => {
        input.addEventListener('input', function (e) {
            // Skip middle initial and position-other (handled separately)
            if (input.id === 'middleInitial' || input.id === 'position-other') return;
            
            const cursorPos = e.target.selectionStart;
            const upperValue = e.target.value.toUpperCase();
            if (e.target.value !== upperValue) {
                e.target.value = upperValue;
                e.target.setSelectionRange(cursorPos, cursorPos);
            }
        });
    });

    // Middle Initial auto-dot (max 1 character)
    if (middleInitialInput) {
        middleInitialInput.addEventListener('input', function (e) {
            let value = e.target.value;
            // Limit to 1 character
            if (value.length > 1) {
                value = value.charAt(0);
            }
            // Auto-capitalize and add dot
            if (value.length > 0) {
                e.target.value = value.charAt(0).toUpperCase() + '.';
            }
        });
    }
}


// Auto-capitalize each word and auto-dot for back panel name fields
function setupBackNameFormatting() {
    // Helper to capitalize each word
    function capitalizeEachWord(value) {
        return value.toLowerCase().split(' ').map(word =>
            word.charAt(0).toUpperCase() + word.slice(1)
        ).join(' ');
    }

    // First Name auto-capitalize each word
    backFirstNameInput.addEventListener('input', function (e) {
        const cursorPos = e.target.selectionStart;
        const value = e.target.value;
        const capitalized = capitalizeEachWord(value);
        if (e.target.value !== capitalized) {
            const diff = capitalized.length - e.target.value.length;
            e.target.value = capitalized;
            e.target.setSelectionRange(cursorPos + diff, cursorPos + diff);
        }
    });

    // Surname auto-capitalize each word
    backSurnameInput.addEventListener('input', function (e) {
        const cursorPos = e.target.selectionStart;
        const value = e.target.value;
        const capitalized = capitalizeEachWord(value);
        if (e.target.value !== capitalized) {
            const diff = capitalized.length - e.target.value.length;
            e.target.value = capitalized;
            e.target.setSelectionRange(cursorPos + diff, cursorPos + diff);
        }
    });

    // Middle Initial auto-capitalize + add dot
    backMiddleInitialInput.addEventListener('input', function (e) {
        const value = e.target.value;
        if (value.length > 0) {
            let formatted = value.charAt(0).toUpperCase();
            if (!formatted.endsWith('.')) {
                formatted += '.';
            }
            if (e.target.value !== formatted) {
                e.target.value = formatted;
            }
        }
    });


    // Address Line 1 auto-capitalize each word
    backAddress1Input.addEventListener('input', function (e) {
        const cursorPos = e.target.selectionStart;
        const value = e.target.value;
        const capitalized = value.toLowerCase().split(' ').map(word =>
            word.charAt(0).toUpperCase() + word.slice(1)
        ).join(' ');
        if (e.target.value !== capitalized) {
            const diff = capitalized.length - e.target.value.length;
            e.target.value = capitalized;
            e.target.setSelectionRange(cursorPos + diff, cursorPos + diff);
        }
    });

    // Address Line 2 auto-capitalize each word
    backAddress2Input.addEventListener('input', function (e) {
        const cursorPos = e.target.selectionStart;
        const value = e.target.value;
        const capitalized = value.toLowerCase().split(' ').map(word =>
            word.charAt(0).toUpperCase() + word.slice(1)
        ).join(' ');
        if (e.target.value !== capitalized) {
            const diff = capitalized.length - e.target.value.length;
            e.target.value = capitalized;
            e.target.setSelectionRange(cursorPos + diff, cursorPos + diff);
        }
    });
}

// Hire date typed as YY / MM / DD, returned as YYYY-MM-DD (20YY), or '' if any part is missing or not a real date
function getHireDate() {
    const yy = backHireYear.value.trim();
    const month = backHireMonth.value.trim().padStart(2, '0');
    const day = backHireDay.value.trim().padStart(2, '0');
    if (!/^\d{2}$/.test(yy) || !/^\d{2}$/.test(month) || !/^\d{2}$/.test(day)) return '';
    const year = `20${yy}`;
    const date = new Date(`${year}-${month}-${day}T00:00:00`);
    if (isNaN(date.getTime()) || date.getMonth() + 1 !== parseInt(month, 10) || date.getDate() !== parseInt(day, 10)) return '';
    return `${year}-${month}-${day}`;
}

// ---------- Printing to the ID card printer (IDP SMART-51 with card flipper: prints both sides in one pass) ----------

const printBtn = document.getElementById('printBtn');
const printArea = document.getElementById('print-area');
const printerStatusBtn = document.getElementById('printerStatus');

// ---- Is the SMART-51 driver installed and the printer plugged in? (asked from Windows by Rust) ----

const PRINTER_STATES = {
    ready: { cls: 'ready', text: p => `${p.printer} ready`, title: p => `Connected: ${p.printer}. Click to check again.` },
    attention: { cls: 'warn', text: p => `Printer: ${p.detail}`, title: p => `${p.printer} is connected but Windows reports: ${p.detail}. Click to check again.` },
    disconnected: { cls: 'warn', text: () => 'Printer not connected', title: p => `The driver is installed (${p.printer}), but the printer is unplugged or turned off. Click to check again.` },
    never_connected: { cls: 'warn', text: () => 'Plug in the printer', title: p => `The driver is installed (${p.driver}), but the printer has not been connected yet. Plug in the USB cable and turn it on - Windows sets it up the first time. Click to check again.` },
    not_installed: { cls: 'error', text: () => 'Printer driver not installed', title: () => 'No SMART-51 printer found in Windows. Install the IDP SMART-51 Windows driver, then click to check again.' },
    unknown: { cls: 'unknown', text: () => 'Printer status unknown', title: p => `${p.error || 'Could not check the printer'}. Click to try again.` }
};

let printerStatus = { state: 'unknown' };
let printerCheck = null;

function renderPrinterStatus() {
    const config = PRINTER_STATES[printerStatus.state] || PRINTER_STATES.unknown;
    printerStatusBtn.className = `printer-status ${config.cls}`;
    printerStatusBtn.querySelector('.printer-status-text').textContent = config.text(printerStatus);
    printerStatusBtn.title = config.title(printerStatus);
}

// One check at a time; callers arriving meanwhile share the running one
function refreshPrinterStatus() {
    if (printerCheck) return printerCheck;
    printerStatusBtn.classList.add('checking');
    printerCheck = ipcRenderer.invoke('get-printer-status')
        .then(result => { printerStatus = result || { state: 'unknown' }; })
        .catch(err => { printerStatus = { state: 'unknown', error: String(err && err.message ? err.message : err) }; })
        .then(() => {
            renderPrinterStatus();
            printerCheck = null;
            return printerStatus;
        });
    return printerCheck;
}

// Message to show when printing is attempted without a ready printer ('' when it is ready)
function printerWarning(status) {
    switch (status.state) {
        case 'ready': return '';
        case 'attention': return `The printer reports "${status.detail}". Check it before printing.`;
        case 'disconnected': return 'The SMART-51 is not connected - plug in the USB cable and turn it on, then choose it in the print dialog.';
        case 'never_connected': return 'The SMART-51 driver is installed, but the printer has never been plugged in. Connect it by USB and turn it on - Windows finishes the setup the first time.';
        case 'not_installed': return 'The SMART-51 driver is not installed, so the card cannot go to the ID printer. Install the driver first.';
        default: return 'Could not check the ID printer. Make sure you choose the SMART-51 in the print dialog.';
    }
}

printerStatusBtn.addEventListener('click', refreshPrinterStatus);
refreshPrinterStatus();

// Re-check when coming back to the app (e.g. after installing the driver or plugging the printer in)
let lastPrinterFocusCheck = Date.now();
window.addEventListener('focus', () => {
    if (Date.now() - lastPrinterFocusCheck < 5000) return;
    lastPrinterFocusCheck = Date.now();
    refreshPrinterStatus();
});

// Prints card sides through the Windows print dialog, one CR80-size page per side (see @media print):
// [front, back] is one two-page job - with two-sided printing on, the printer puts page 2 on the back of the card.
// The printer is checked first; the dialog still opens either way, and the returned warning ('' if ready) is shown by the caller.
async function printCard(pages) {
    const warning = printerWarning(await refreshPrinterStatus());
    printArea.innerHTML = '';
    const images = pages.map(svgData => {
        const page = document.createElement('div');
        page.className = 'print-page';
        const img = new Image();
        img.src = svgData;
        img.alt = '';
        page.appendChild(img);
        printArea.appendChild(page);
        return img;
    });
    try {
        await Promise.all(images.map(img => img.decode())); // make sure the cards are drawn before the print snapshot
    } catch (err) {
        console.error('Card image could not be prepared for printing:', err);
    }
    // Wait until the print dialog is closed (Print and Cancel both end with "afterprint"). window.print()
    // normally blocks until then; the short fallback covers a WebView that returns early without the event.
    let printed = false;
    const dialogClosed = new Promise(resolve => {
        window.addEventListener('afterprint', () => { printed = true; resolve(); }, { once: true });
    });
    window.print();
    await Promise.race([dialogClosed, new Promise(resolve => setTimeout(resolve, 1000))]);
    if (!printed) console.warn('No afterprint event after window.print()');
    return warning;
}

window.addEventListener('afterprint', () => {
    printArea.innerHTML = '';
});

// ---- "Did the card print?" - the only way to know, since the dialog does not report Print vs Cancel ----

const printConfirmModal = document.getElementById('printConfirmModal');
let resolvePrintConfirm = null;

// what: 'card' (both sides), 'front' or 'back'
function askDidPrint(what) {
    document.getElementById('printConfirmTitle').textContent = what === 'card' ? 'Did the card print?' : `Did the ${what} of the card print?`;
    printConfirmModal.style.display = 'flex';
    document.getElementById('printConfirmYesBtn').focus();
    return new Promise(resolve => { resolvePrintConfirm = resolve; });
}

function closePrintConfirm(printed) {
    printConfirmModal.style.display = 'none';
    if (resolvePrintConfirm) resolvePrintConfirm(printed);
    resolvePrintConfirm = null;
}

document.getElementById('printConfirmYesBtn').addEventListener('click', () => closePrintConfirm(true));
document.getElementById('printConfirmNoBtn').addEventListener('click', () => closePrintConfirm(false));
printConfirmModal.addEventListener('keydown', e => {
    if (e.key === 'Escape') { e.stopPropagation(); closePrintConfirm(false); }
});

// What the bottom-bar Print button prints: both sides in one job once the front and back of the same card
// are downloaded, otherwise just this tab's side
function actionBarPrintJob() {
    const { front, back } = lastGenerated;
    if (front && back && front.control && front.control === back.control) {
        return { controlNumber: front.control, sides: [{ side: 'front', svg: front.svg }, { side: 'back', svg: back.svg }] };
    }
    const own = (activeTab === 'front' || activeTab === 'back') && lastGenerated[activeTab];
    return own ? { controlNumber: own.control, sides: [{ side: activeTab, svg: own.svg }] } : null;
}

function updatePrintButton() {
    if (activeTab !== 'front' && activeTab !== 'back') return;
    const job = actionBarPrintJob();
    const both = job && job.sides.length === 2;
    const side = activeTab === 'back' ? 'Back' : 'Front';
    printBtn.querySelector('span').textContent = both ? 'Print Both Sides' : `Print ${side}`;
    printBtn.disabled = !job;
    printBtn.title = both ? 'Print the front and back of the card in one go'
        : job ? `Print the ${side.toLowerCase()} only - download the other side too to print both at once`
        : 'Download the card first, then print it';
}

// Editing a tab after downloading makes its saved card out of date, so it has to be downloaded again before printing
['front', 'back'].forEach(tab => {
    const panel = document.getElementById(`${tab}-panel`);
    const invalidate = e => {
        // Typing in a dropdown's search box is not an edit to the card
        if (e && e.target.closest && e.target.closest('.select-search')) return;
        if (!lastGenerated[tab]) return;
        lastGenerated[tab] = null;
        updatePrintButton();
    };
    panel.addEventListener('input', invalidate);
    panel.addEventListener('change', invalidate);
    panel.addEventListener('click', e => {
        if (e.target.closest('.option, .seg-btn')) invalidate();
    });
});

// ---- Print log: every card side sent to the printer is recorded; reprints ask for a reason first ----

const reprintModal = document.getElementById('reprintModal');
const reprintConfirmBtn = document.getElementById('reprintConfirmBtn');
const reprintNote = document.getElementById('reprintNote');
const reasonButtons = reprintModal.querySelectorAll('.reason-btn');
let resolveReprint = null;
let chosenReason = '';

function formatPrintDate(iso) {
    const date = new Date(iso);
    return `${date.toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' })} ${date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
}

// Resolves with the reason text ("Lost", "Other: left in bus"...) or null when the operator cancels.
// what: 'card' (both sides), 'front' or 'back'; previous: earlier print_log entries of those sides, newest first
function askReprintReason(what, previous) {
    const counts = ['front', 'back']
        .map(side => [side, previous.filter(e => e.side === side).length])
        .filter(([, n]) => n > 0)
        .map(([side, n]) => `${side} ${n === 1 ? 'once' : `${n} times`}`)
        .join(', ');
    document.getElementById('reprintTitle').textContent = what === 'card' ? 'Reprint this card?' : `Reprint ${what} of this card?`;
    document.getElementById('reprintDescription').textContent =
        `This card was already sent to the printer (${counts}, last on ${formatPrintDate(previous[0].printedAt)}).` +
        ' Choose a reason for the reprint.';
    chosenReason = '';
    reasonButtons.forEach(btn => btn.classList.remove('active'));
    reprintNote.value = '';
    reprintConfirmBtn.disabled = true;
    reprintModal.style.display = 'flex';
    reasonButtons[0].focus();
    return new Promise(resolve => { resolveReprint = resolve; });
}

function closeReprint(result) {
    reprintModal.style.display = 'none';
    if (resolveReprint) resolveReprint(result);
    resolveReprint = null;
}

reasonButtons.forEach(btn => {
    btn.addEventListener('click', () => {
        chosenReason = btn.dataset.reason;
        reasonButtons.forEach(b => b.classList.toggle('active', b === btn));
        reprintConfirmBtn.disabled = false;
        reprintNote.focus();
    });
});

reprintConfirmBtn.addEventListener('click', () => {
    const note = reprintNote.value.trim();
    closeReprint(note ? `${chosenReason}: ${note}` : chosenReason);
});
document.getElementById('reprintCancelBtn').addEventListener('click', () => closeReprint(null));
reprintModal.addEventListener('click', e => { if (e.target === reprintModal) closeReprint(null); });
reprintModal.addEventListener('keydown', e => {
    if (e.key === 'Escape') { e.stopPropagation(); closeReprint(null); }
    // Enter in the details box confirms; on a reason button Enter just picks that reason
    if (e.key === 'Enter' && e.target === reprintNote && !reprintConfirmBtn.disabled) reprintConfirmBtn.click();
});

// Prints one or both sides of a card as one job and logs each side once the operator confirms it printed.
// sides: [{ side: 'front'|'back', svg }] - front first, so with both sides the back is page 2.
// Returns { cancelled } when the reprint question was cancelled, { notPrinted } when the operator said it
// did not print (nothing is logged), otherwise { warning } ('' when the ID printer was ready).
async function printCardSides(controlNumber, sides) {
    const what = sides.length === 2 ? 'card' : sides[0].side;
    let reason = '';
    if (controlNumber) {
        try {
            const result = await ipcRenderer.invoke('get-print-history', controlNumber);
            const previous = (result.history || []).filter(entry => sides.some(s => s.side === entry.side));
            if (previous.length) {
                reason = await askReprintReason(what, previous);
                if (reason === null) return { cancelled: true };
            }
        } catch (err) {
            console.error('Could not read the print history:', err);
        }
    }

    const warning = await printCard(sides.map(s => s.svg));
    if (!(await askDidPrint(what))) return { notPrinted: true };

    if (controlNumber) {
        const printer = printerStatus.state === 'ready' ? printerStatus.printer : '';
        for (const { side } of sides) {
            try {
                const logged = await ipcRenderer.invoke('log-print', controlNumber, side, reason, printer);
                if (!logged.success) console.error('Print was not logged:', logged.error);
            } catch (err) {
                console.error('Print was not logged:', err);
            }
        }
        if (activeTab === 'records') loadRecords();
        if (activeTab === 'printlog') loadPrintLog();
    }
    return { warning };
}

printBtn.addEventListener('click', async () => {
    const job = actionBarPrintJob();
    if (!job) return;
    const { cancelled, notPrinted, warning } = await printCardSides(job.controlNumber, job.sides);
    if (cancelled) return;
    if (notPrinted) {
        status.innerHTML = '<i class="fas fa-info-circle"></i> <span id="status-text">Print cancelled or failed - nothing was recorded in the print log.</span>';
        status.className = 'status info';
        return;
    }
    if (warning) {
        status.innerHTML = `<i class="fas fa-exclamation-triangle"></i> <span id="status-text">${escapeHtml(warning)}</span>`;
        status.className = 'status error';
        return;
    }
    const printed = job.sides.length === 2 ? 'Front and back' : job.sides[0].side === 'back' ? 'Back' : 'Front';
    status.innerHTML = `<i class="fas fa-check-circle"></i> <span id="status-text">${printed} printed and recorded in the print log.</span>`;
    status.className = 'status success';
});

function checkReady() {
    updatePrintButton();
    if (activeTab === 'back') {
        downloadBtn.disabled = !dataUpdated;
    } else {
        downloadBtn.disabled = !dataUpdated;
    }
    const downloadSpan = downloadBtn.querySelector('span');
    if (downloadSpan) {
        downloadSpan.textContent = dataUpdated ? 'Download SVG for Corel' : 'Update Data First';
    }
}

// ---------- Contact number: Mobile (+63 9XX XXX XXXX) or Telephone ((02) 8XXX XXXX) ----------

const phoneInputBox = document.getElementById('phone-input');
const phonePrefix = document.getElementById('phone-prefix');
const phoneHint = document.getElementById('phone-hint');
const phoneTypeButtons = document.querySelectorAll('[data-phone-type]');
let phoneType = 'mobile';

const PHONE_TYPES = {
    mobile: {
        format: formatMobileInput,
        isValid: isValidMobileInput,
        placeholder: '917 123 4567',
        maxLength: 12,
        hint: '10-digit mobile number starting with 9',
        error: 'Must be 10 digits starting with 9, e.g. 917 123 4567'
    },
    landline: {
        format: formatLandlineInput,
        isValid: isValidLandlineInput,
        placeholder: '(02) 8123 4567',
        maxLength: 14,
        hint: 'Include the area code: (02) for Metro Manila, e.g. (032) for Cebu',
        error: 'Include the area code, e.g. (02) 8123 4567 or (032) 234 5678'
    }
};

// Put the caret back after the same number of digits it followed before reformatting
function caretAfterDigits(value, digitCount) {
    if (digitCount <= 0) return value.search(/\d/) === -1 ? value.length : value.search(/\d/);
    let seen = 0;
    for (let i = 0; i < value.length; i++) {
        if (/\d/.test(value[i]) && ++seen === digitCount) return i + 1;
    }
    return value.length;
}

function reformatContact(caretDigits) {
    const formatted = PHONE_TYPES[phoneType].format(backContactInput.value);
    const digitsBefore = backContactInput.value.replace(/\D/g, '').length;
    const digitsAfter = formatted.replace(/\D/g, '').length;
    backContactInput.value = formatted;
    // A pasted 0 / 63 prefix was dropped, so digit positions no longer line up - go to the end
    const caret = digitsAfter < digitsBefore ? formatted.length : caretAfterDigits(formatted, caretDigits);
    if (document.activeElement === backContactInput) backContactInput.setSelectionRange(caret, caret);
}

function showContactHint(showError) {
    const type = PHONE_TYPES[phoneType];
    phoneHint.textContent = showError ? type.error : type.hint;
    phoneHint.classList.toggle('error', showError);
    phoneInputBox.classList.toggle('invalid', showError);
}

// Empty is allowed (the card then shows the PHONE NUMBER placeholder)
function validateContactNumber() {
    const value = backContactInput.value.trim();
    const valid = !value || PHONE_TYPES[phoneType].isValid(value);
    showContactHint(!valid);
    return valid;
}

// Value sent to Rust: mobile as +63 9XX XXX XXXX (Rust prints +63 9XX-XXX-XXXX), telephone as typed
function getContactNumber() {
    const value = backContactInput.value.trim();
    if (!value) return '';
    return phoneType === 'mobile' ? `+63 ${value}` : value;
}

function setPhoneType(type) {
    phoneType = type;
    const config = PHONE_TYPES[type];
    phoneTypeButtons.forEach(btn => {
        const active = btn.dataset.phoneType === type;
        btn.classList.toggle('active', active);
        btn.setAttribute('aria-checked', String(active));
    });
    phonePrefix.hidden = type !== 'mobile';
    backContactInput.placeholder = config.placeholder;
    backContactInput.maxLength = config.maxLength;
    // A mobile number means nothing as a landline (and vice versa), so only keep a number that fits the new type
    backContactInput.value = config.isValid(backContactInput.value) ? config.format(backContactInput.value) : '';
    showContactHint(false);
}

phoneTypeButtons.forEach(btn => {
    btn.addEventListener('click', () => {
        setPhoneType(btn.dataset.phoneType);
        backContactInput.focus();
    });
});

backContactInput.addEventListener('input', () => {
    const caretDigits = backContactInput.value.slice(0, backContactInput.selectionStart).replace(/\D/g, '').length;
    reformatContact(caretDigits);
    if (phoneInputBox.classList.contains('invalid')) validateContactNumber();
});

// Backspace right after a space or bracket deletes the digit before it instead of getting stuck
backContactInput.addEventListener('keydown', e => {
    if (e.key !== 'Backspace') return;
    const { selectionStart: start, selectionEnd: end, value } = backContactInput;
    if (start !== end || start === 0 || /\d/.test(value[start - 1])) return;
    e.preventDefault();
    const digitsBefore = value.slice(0, start).replace(/\D/g, '').length;
    if (digitsBefore === 0) return;
    const digits = value.replace(/\D/g, '');
    backContactInput.value = digits.slice(0, digitsBefore - 1) + digits.slice(digitsBefore);
    reformatContact(digitsBefore - 1);
});

// Paste is handled here because maxlength would cut "+63 917 123 4567" short before it is reformatted.
// A mobile number pasted while in Telephone mode switches to Mobile.
backContactInput.addEventListener('paste', e => {
    e.preventDefault();
    const pasted = (e.clipboardData || window.clipboardData).getData('text');
    if (phoneType === 'landline' && isValidMobileInput(pasted)) {
        backContactInput.value = pasted;
        setPhoneType('mobile');
        return;
    }
    const { selectionStart: start, selectionEnd: end, value } = backContactInput;
    backContactInput.value = value.slice(0, start) + pasted + value.slice(end);
    backContactInput.value = PHONE_TYPES[phoneType].format(backContactInput.value);
    if (phoneInputBox.classList.contains('invalid')) validateContactNumber();
});

backContactInput.addEventListener('blur', validateContactNumber);

// ID Picture handling
idPicInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = function (event) {
        idPicBase64 = event.target.result;
        preview.src = idPicBase64;
        preview.style.display = 'block';
        updatePlaceholder(idPlaceholder, true);
        checkReady();
    };
    reader.readAsDataURL(file);
});

sigInput.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = function (event) {
        sigBase64 = event.target.result;
        sigPreview.src = sigBase64;
        sigPreview.style.display = 'block';
        updatePlaceholder(sigPlaceholder, true);
        checkReady();
    };
    reader.readAsDataURL(file);
});

idDropArea.addEventListener('click', () => idPicInput.click());
sigDropArea.addEventListener('click', () => sigInput.click());

idDropArea.addEventListener('dragover', (e) => {
    e.preventDefault();
    idDropArea.classList.add('dragover');
});

idDropArea.addEventListener('dragleave', () => {
    idDropArea.classList.remove('dragover');
});

idDropArea.addEventListener('drop', (e) => {
    e.preventDefault();
    idDropArea.classList.remove('dragover');
    const file = e.dataTransfer.files[0];
    if (file && file.type.startsWith('image/')) {
        const reader = new FileReader();
        reader.onload = function (event) {
            idPicBase64 = event.target.result;
            preview.src = idPicBase64;
            preview.style.display = 'block';
            updatePlaceholder(idPlaceholder, true);
            checkReady();
        };
        reader.readAsDataURL(file);
    }
});

sigDropArea.addEventListener('dragover', (e) => {
    e.preventDefault();
    sigDropArea.classList.add('dragover');
});

sigDropArea.addEventListener('dragleave', () => {
    sigDropArea.classList.remove('dragover');
});

sigDropArea.addEventListener('drop', (e) => {
    e.preventDefault();
    sigDropArea.classList.remove('dragover');
    const file = e.dataTransfer.files[0];
    if (file && file.type.startsWith('image/')) {
        const reader = new FileReader();
        reader.onload = function (event) {
            sigBase64 = event.target.result;
            sigPreview.src = sigBase64;
            sigPreview.style.display = 'block';
            updatePlaceholder(sigPlaceholder, true);
            checkReady();
        };
        reader.readAsDataURL(file);
    }
});

updateBtn.addEventListener('click', () => {
    dataUpdated = true;
    status.innerHTML = '<i class="fas fa-check-circle"></i> <span id="status-text">Data updated! Ready to download.</span>';
    status.className = 'status success';
    checkReady();
});

downloadBtn.addEventListener('click', async () => {
    downloadBtn.disabled = true;
    status.innerHTML = '<i class="fas fa-spinner fa-spin"></i> <span id="status-text">Generating SVG...</span>';
    status.className = 'status loading';

    let data;

    if (activeTab === 'front') {
        // Front ID data - use other input if OTHERS selected
        let positionValue = positionInput.value.trim();
        if (positionValue === 'OTHERS' && positionOtherInput.value.trim()) {
            positionValue = positionOtherInput.value.trim();
        }
 data = {
             idPicture: idPicBase64,
             signaturePicture: sigBase64,
             controlNumber: issuedControlNumber,
             lastName: lastNameInput.value.trim() || 'LAST NAME',
             suffix: suffixInput.value.trim(),
             firstName: firstNameInput.value.trim() || 'FIRST NAME',
             middleInitial: middleInitialInput.value.trim(),
             position: positionValue || 'SECURITY GUARD',
             isFront: activeTab === 'front',
             hireDate: getHireDate(),
             cityOfBirth: citySelect ? citySelect.value : '',
             isRehire: backRehire ? backRehire.checked : false
          };
    } else {
        if (!validateContactNumber()) {
            const message = phoneType === 'mobile'
                ? 'Contact number is not a valid mobile number - it must be 10 digits starting with 9, like +63 917 123 4567'
                : 'Contact number is not a valid telephone number - include the area code, like (02) 8123 4567 or (032) 234 5678';
            status.innerHTML = `<i class="fas fa-exclamation-circle"></i> <span id="status-text">${message}</span>`;
            status.className = 'status error';
            backContactInput.focus();
            downloadBtn.disabled = false;
            return;
        }

        const frontLastName = lastNameInput.value.trim() || 'LAST NAME';
        const frontFirstName = firstNameInput.value.trim() || 'FIRST NAME';
        
        data = {
            name: ([
                backFirstNameInput.value.trim(),
                backMiddleInitialInput.value.trim(),
                backSurnameInput.value.trim(),
                backSuffixInput.value.trim()
            ].filter(Boolean).join(' ')).trim() || 'FULL NAME',
            addressLine1: backAddress1Input.value.trim() || '',
            addressLine2: backAddress2Input.value.trim() || '',
            relationship: backRelationshipInput.value.trim() || 'RELATIONSHIP',
            contact: getContactNumber() || 'PHONE NUMBER',
            surname: frontLastName,
            firstName: frontFirstName,
            isFront: false,
            controlNumber: issuedControlNumber,
            hireDate: getHireDate(),
            cityOfBirth: citySelect ? citySelect.value : '',
            isRehire: backRehire ? backRehire.checked : false
        };
    }

    try {
        const result = await ipcRenderer.invoke('generate-svg', data);
        if (result.success) {
            if (result.controlNumber) issuedControlNumber = result.controlNumber;
            // The back is filed under the control number issued with the front
            lastGenerated[activeTab] = { svg: result.svgData, control: issuedControlNumber };
            updatePrintButton();
            status.innerHTML = `<i class="fas fa-check-circle"></i> <span id="status-text">SVG saved to: ${result.savePath}</span>`;
            status.className = 'status success';
        } else {
            status.innerHTML = `<i class="fas fa-exclamation-circle"></i> <span id="status-text">Error: ${result.error}</span>`;
            status.className = 'status error';
        }
    } catch (err) {
        status.innerHTML = `<i class="fas fa-exclamation-circle"></i> <span id="status-text">Error: ${err.message}</span>`;
        status.className = 'status error';
    }
    downloadBtn.disabled = false;
});

// Tab switching functionality
document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        // Remove active class from all tabs and panels
        document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
        document.querySelectorAll('.tab-panel').forEach(panel => panel.classList.remove('active'));

        // Add active class to clicked tab and corresponding panel
        btn.classList.add('active');
        document.getElementById(btn.dataset.tab + '-panel').classList.add('active');

        // Update active tab tracker
        activeTab = btn.dataset.tab;

        // Update status message and button visibility
        const btnRow = document.querySelector('.btn-row');
        if (activeTab === 'records') {
            status.innerHTML = '<i class="fas fa-info-circle"></i> <span id="status-text">Viewing registered employee records</span>';
            if (btnRow) btnRow.style.display = 'none';
            loadRecords();
        } else if (activeTab === 'printlog') {
            status.innerHTML = '<i class="fas fa-info-circle"></i> <span id="status-text">Viewing the print log</span>';
            if (btnRow) btnRow.style.display = 'none';
            loadPrintLog();
        } else {
            status.innerHTML = '<i class="fas fa-info-circle"></i> <span id="status-text">Fill in the information and upload images</span>';
            if (btnRow) btnRow.style.display = 'flex';
        }
        status.className = 'status info';
        checkReady();
    });
    
    // Reset database button
    const resetRecordsBtn = document.getElementById('resetRecordsBtn');
    const resetModal = document.getElementById('resetModal');
    const resetPinModal = document.getElementById('resetPinModal');
    const modalCancelBtn = document.getElementById('modalCancelBtn');
    const modalConfirmBtn = document.getElementById('modalConfirmBtn');
    const resetPinCancelBtn = document.getElementById('resetPinCancelBtn');
    const resetPinConfirmBtn = document.getElementById('resetPinConfirmBtn');
    const resetPinInputs = document.getElementById('resetPinInputs');
    const resetPinError = document.getElementById('resetPinError');
    
    if (resetRecordsBtn && resetPinModal) {
        resetRecordsBtn.addEventListener('click', () => {
            resetPinError.textContent = '';
            const pinBoxes = resetPinInputs.querySelectorAll('.pin-box');
            pinBoxes.forEach(box => box.value = '');
            resetPinModal.style.display = 'flex';
            pinBoxes[0].focus();
        });
    }
    
    if (modalCancelBtn && resetModal) {
        modalCancelBtn.addEventListener('click', () => {
            resetModal.style.display = 'none';
        });
    }
    
    if (resetPinCancelBtn && resetPinModal) {
        resetPinCancelBtn.addEventListener('click', () => {
            resetPinModal.style.display = 'none';
        });
    }
    
    if (resetPinConfirmBtn && resetPinModal) {
        resetPinConfirmBtn.addEventListener('click', async () => {
            const pinBoxes = resetPinInputs.querySelectorAll('.pin-box');
            const pin = Array.from(pinBoxes).map(box => box.value).join('');
            
            if (pin.length !== 4) {
                resetPinError.textContent = 'Please enter a 4-digit PIN';
                return;
            }
            
            const result = await ipcRenderer.invoke('verify-pin', pin);
            if (result.success) {
                resetPinModal.style.display = 'none';
                resetModal.style.display = 'flex';
            } else {
                resetPinError.textContent = 'Incorrect PIN';
                pinBoxes.forEach(box => {
                    box.value = '';
                    box.classList.add('error');
                });
                setTimeout(() => {
                    pinBoxes.forEach(box => box.classList.remove('error'));
                }, 400);
                pinBoxes[0].focus();
            }
        });
    }
    
    if (modalConfirmBtn && resetModal) {
        modalConfirmBtn.addEventListener('click', async () => {
            resetModal.style.display = 'none';
            
            const result = await ipcRenderer.invoke('reset-database');
            if (result.success) {
                status.innerHTML = '<i class="fas fa-check-circle"></i> <span id="status-text">Database reset successfully</span>';
                status.className = 'status success';
                loadRecords();
            } else {
                status.innerHTML = `<i class="fas fa-exclamation-circle"></i> <span id="status-text">Error resetting database: ${result.error}</span>`;
                status.className = 'status error';
            }
        });
    }
    
    if (resetPinInputs) {
        const pinBoxes = resetPinInputs.querySelectorAll('.pin-box');
        pinBoxes.forEach((box, index) => {
            box.addEventListener('input', function(e) {
                if (box.value.length === 1 && index < pinBoxes.length - 1) {
                    pinBoxes[index + 1].focus();
                }
                resetPinError.textContent = '';
            });

            box.addEventListener('keydown', function(e) {
                if (e.key === 'Backspace' && box.value === '' && index > 0) {
                    pinBoxes[index - 1].focus();
                }
                if (e.key === 'Enter') {
                    resetPinConfirmBtn.click();
                }
            });

            box.addEventListener('paste', function(e) {
                e.preventDefault();
                const paste = (e.clipboardData || window.clipboardData).getData('text').slice(0, 4);
                const digits = paste.replace(/\D/g, '').split('');
                digits.forEach((digit, i) => {
                    if (pinBoxes[i]) pinBoxes[i].value = digit;
                });
                if (digits.length > 0 && digits.length < 4) {
                    pinBoxes[digits.length].focus();
                } else if (digits.length === 4) {
                    resetPinConfirmBtn.click();
                }
            });
        });
    }
    
    const recordsSearchInput = document.getElementById('records-search');
    if (recordsSearchInput) {
        recordsSearchInput.addEventListener('input', function() {
            recordsSearchText = this.value;
            if (activeTab === 'records') {
                loadRecords();
            }
        });
    }
    
    const recordsTableHeaders = document.querySelectorAll('#records-table th[data-sort]');
    recordsTableHeaders.forEach(th => {
        th.addEventListener('click', function() {
            const column = this.dataset.sort;
            if (recordsSortColumn === column) {
                recordsSortDirection = recordsSortDirection === 'asc' ? 'desc' : 'asc';
            } else {
                recordsSortColumn = column;
                recordsSortDirection = 'asc';
            }
            if (activeTab === 'records') {
                loadRecords();
            }
        });
    });
});

async function loadRecords() {
    const recordsTableBody = document.querySelector('#records-table tbody');
    if (!recordsTableBody) return;
    
    try {
        const [records, printSummary] = await Promise.all([
            ipcRenderer.invoke('get-all-records'),
            ipcRenderer.invoke('get-print-summary').catch(() => ({}))
        ]);

        const mergedMap = new Map();
        
        records.forEach(r => {
            // New records are linked by control number; older ones by the former ID number
            const id = r.control_number || r.id_number || 'UNKNOWN';
            if (!mergedMap.has(id)) {
                mergedMap.set(id, {
                    fullName: '',
                    emergencyName: '',
                    position: '',
                    hireDate: '',
                    cityOfBirth: '',
                    address1: '',
                    address2: '',
                    relationship: '',
                    contact: '',
                    controlNumber: '',
                    isRehire: false,
                    createdAt: r.created_at,
                    hasFront: false,
                    hasBack: false
                });
            }
            
            const item = mergedMap.get(id);
            
            if (r.type === 'back') {
                item.emergencyName = item.emergencyName || r.employee_name || '';
                item.hireDate = item.hireDate || r.hire_date || '';
                item.cityOfBirth = item.cityOfBirth || r.city_of_birth || '';
                item.address1 = item.address1 || r.address1 || '';
                item.address2 = item.address2 || r.address2 || '';
                item.relationship = item.relationship || r.relationship || '';
                item.contact = item.contact || r.contact || '';
                item.controlNumber = item.controlNumber || r.control_number || '';
                if (r.is_rehire) item.isRehire = true;
                item.hasBack = true;
            }
            
            if (r.type === 'front') {
                const frontName = [r.first_name, r.middle_initial, r.last_name, r.suffix].filter(Boolean).join(' ');
                item.fullName = item.fullName || frontName || '';
                item.position = item.position || r.position || '';
                item.hireDate = item.hireDate || r.hire_date || '';
                item.cityOfBirth = item.cityOfBirth || r.city_of_birth || '';
                item.controlNumber = item.controlNumber || r.control_number || '';
                if (r.is_rehire) item.isRehire = true;
                item.hasFront = true;
            }

            if (!item.createdAt || new Date(r.created_at) < new Date(item.createdAt)) {
                item.createdAt = r.created_at;
            }
        });
        
        let mergedRecords = Array.from(mergedMap.values());

        // Times each side was sent to the printer (print_log), keyed by control number
        mergedRecords.forEach(r => {
            const printed = (r.controlNumber && printSummary[r.controlNumber]) || {};
            r.printFront = printed.front || 0;
            r.printBack = printed.back || 0;
            r.printCount = r.printFront + r.printBack;
            r.lastPrintedAt = printed.lastPrintedAt || '';
        });
        
        if (recordsSearchText.trim()) {
            const term = recordsSearchText.trim().toLowerCase();
            mergedRecords = mergedRecords.filter(r => {
                return [
                    r.fullName,
                    r.position,
                    r.hireDate,
                    r.cityOfBirth,
                    r.emergencyName,
                    r.relationship,
                    r.contact,
                    r.address1,
                    r.address2,
                    r.controlNumber,
                    r.isRehire ? 'Yes' : 'No'
                ].some(val => String(val || '').toLowerCase().includes(term));
            });
        }
        
        mergedRecords.sort((a, b) => {
            let aVal = a[recordsSortColumn];
            let bVal = b[recordsSortColumn];
            
            if (recordsSortColumn === 'printCount') {
                aVal = a.printCount;
                bVal = b.printCount;
            } else if (recordsSortColumn === 'no' || recordsSortColumn === 'isRehire') {
                aVal = recordsSortColumn === 'no' ? 0 : (a.isRehire ? 1 : 0);
                bVal = recordsSortColumn === 'no' ? 0 : (b.isRehire ? 1 : 0);
            } else if (recordsSortColumn === 'createdAt') {
                aVal = a.createdAt ? new Date(a.createdAt).getTime() : 0;
                bVal = b.createdAt ? new Date(b.createdAt).getTime() : 0;
            } else {
                aVal = String(aVal || '').toLowerCase();
                bVal = String(bVal || '').toLowerCase();
            }
            
            if (aVal < bVal) return recordsSortDirection === 'asc' ? -1 : 1;
            if (aVal > bVal) return recordsSortDirection === 'asc' ? 1 : -1;
            return 0;
        });
        
        recordsTableBody.innerHTML = '';
        const recordsCount = document.getElementById('records-count');
        if (recordsCount) recordsCount.textContent = mergedRecords.length;

        if (mergedRecords.length === 0) {
            if (recordsSearchText.trim()) {
                showRecordsEmpty('No matching records', `Nothing matches "${recordsSearchText.trim()}".`);
            } else {
                showRecordsEmpty('No records yet', 'Generated IDs will appear here.');
            }
            updateSortIndicators();
            return;
        }
        hideRecordsEmpty();

        const dash = '<span class="cell-empty">—</span>';
        const cell = value => value ? escapeHtml(value) : dash;

        mergedRecords.forEach((r, idx) => {
            const tr = document.createElement('tr');
            const created = r.createdAt ? new Date(r.createdAt) : null;
            const address = [r.address1, r.address2].filter(Boolean).map(escapeHtml).join('<br>');
            tr.innerHTML = `
                <td class="cell-num">${idx + 1}</td>
                <td>${created ? `${created.toLocaleDateString()}<span class="cell-sub">${created.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>` : dash}</td>
                <td class="cell-name">${cell(r.fullName)}</td>
                <td>${cell(r.position)}</td>
                <td>${r.controlNumber ? `<span class="control-chip">${escapeHtml(r.controlNumber)}</span>` : dash}</td>
                <td>${cell(r.hireDate)}</td>
                <td>${cell(r.cityOfBirth)}</td>
                <td>${r.isRehire ? '<span class="badge badge-rehire">Rehire</span>' : '<span class="badge badge-muted">No</span>'}</td>
                <td class="cell-cards">${r.hasFront ? '<span class="card-chip front">Front</span>' : ''}${r.hasBack ? '<span class="card-chip back">Back</span>' : ''}</td>
                <td class="cell-printed">${r.printCount
                    ? `<span class="printed-count${r.printFront > 1 || r.printBack > 1 ? ' reprinted' : ''}" title="Front ${r.printFront}× · Back ${r.printBack}×">`
                        + `<i class="fas fa-print"></i> F${r.printFront} · B${r.printBack}</span>`
                        + `<span class="cell-sub">${escapeHtml(new Date(r.lastPrintedAt).toLocaleDateString([], { day: 'numeric', month: 'short' }))}</span>`
                    : '<span class="cell-empty">Not printed</span>'}</td>
                <td class="emergency-cell cell-name">${cell(r.emergencyName)}</td>
                <td class="emergency-cell">${r.relationship ? `<span class="badge badge-relationship">${escapeHtml(r.relationship)}</span>` : dash}</td>
                <td class="emergency-cell">${cell(r.contact)}</td>
                <td class="emergency-cell cell-address">${address || dash}</td>
                <td class="cell-actions">
                    <button class="view-btn" data-control="${escapeHtml(r.controlNumber)}" data-name="${escapeHtml(r.fullName || r.emergencyName)}"
                        ${r.controlNumber ? '' : 'disabled title="No control number"'}>
                        <i class="fas fa-eye"></i> View
                    </button>
                </td>
            `;
            recordsTableBody.appendChild(tr);
        });

        updateSortIndicators();
    } catch (err) {
        console.error('Failed to load records:', err);
        showRecordsEmpty('Could not load records', String(err && err.message ? err.message : err));
        updateSortIndicators();
    }
}

// ---------- ID preview (Records > View) ----------

const previewModal = document.getElementById('previewModal');
const previewTitle = document.getElementById('previewTitle');
const previewControl = document.getElementById('previewControl');
const previewFront = document.getElementById('previewFront');
const previewBack = document.getElementById('previewBack');
const previewFolderBtn = document.getElementById('previewFolderBtn');
const printFrontBtn = document.getElementById('printFrontBtn');
const printBackBtn = document.getElementById('printBackBtn');
const printBothBtn = document.getElementById('printBothBtn');
let previewControlNumber = '';
let previewCards = { front: null, back: null };

function resetPreviewPrinting(cards) {
    previewCards = cards;
    printFrontBtn.disabled = !cards.front;
    printBackBtn.disabled = !cards.back;
    printBothBtn.disabled = !(cards.front && cards.back);
    showPrintWarning('');
}

function setPreviewFrame(frame, state, side) {
    if (state === 'loading') {
        frame.innerHTML = '<div class="preview-placeholder"><i class="fas fa-spinner fa-spin"></i><span>Loading...</span></div>';
    } else if (state && state.svgData) {
        // <img> renders the SVG without running anything inside it
        frame.innerHTML = '';
        const img = document.createElement('img');
        img.src = state.svgData;
        img.alt = `${side} ID`;
        img.title = state.path;
        frame.appendChild(img);
    } else {
        frame.innerHTML = `<div class="preview-placeholder"><i class="fas fa-file-excel"></i><span>${side} ID not generated yet, or its file was moved</span></div>`;
    }
}

async function openCardPreview(controlNumber, name) {
    previewControlNumber = controlNumber;
    previewTitle.textContent = name || 'ID Preview';
    previewControl.textContent = controlNumber;
    setPreviewFrame(previewFront, 'loading', 'Front');
    setPreviewFrame(previewBack, 'loading', 'Back');
    resetPreviewPrinting({ front: null, back: null });
    previewModal.style.display = 'flex';
    document.getElementById('previewCloseBtn').focus();

    try {
        const result = await ipcRenderer.invoke('get-card-preview', controlNumber);
        if (previewControlNumber !== controlNumber) return; // another record was opened meanwhile
        if (!result.success) throw new Error(result.error);
        setPreviewFrame(previewFront, result.front, 'Front');
        setPreviewFrame(previewBack, result.back, 'Back');
        previewFolderBtn.disabled = !result.front && !result.back;
        resetPreviewPrinting({ front: result.front && result.front.svgData, back: result.back && result.back.svgData });
        loadPrintHistory(controlNumber);
    } catch (err) {
        const message = `<div class="preview-placeholder error"><i class="fas fa-exclamation-circle"></i><span>${escapeHtml(err.message || err)}</span></div>`;
        previewFront.innerHTML = message;
        previewBack.innerHTML = '';
        previewFolderBtn.disabled = true;
    }
}

function showPrintWarning(warning) {
    document.getElementById('printWarningText').textContent = warning;
    document.getElementById('printWarning').hidden = !warning;
}

// Print history of the card shown in the preview, newest first
async function loadPrintHistory(controlNumber) {
    const list = document.getElementById('printHistoryList');
    list.innerHTML = '';
    try {
        const result = await ipcRenderer.invoke('get-print-history', controlNumber);
        if (previewControlNumber !== controlNumber) return;
        const history = result.history || [];
        if (!history.length) {
            list.innerHTML = '<li class="print-history-empty">Not printed yet</li>';
            return;
        }
        list.innerHTML = history.map(entry => `
            <li>
                <span class="card-chip ${entry.side === 'back' ? 'back' : 'front'}">${entry.side === 'back' ? 'Back' : 'Front'}</span>
                <span class="print-history-date">${escapeHtml(formatPrintDate(entry.printedAt))}</span>
                ${entry.isReprint
                    ? `<span class="badge badge-rehire">Reprint</span>${entry.reason ? `<span class="print-history-reason">${escapeHtml(entry.reason)}</span>` : ''}`
                    : '<span class="badge badge-muted">First print</span>'}
                ${entry.printer ? `<span class="print-history-printer"><i class="fas fa-print"></i> ${escapeHtml(entry.printer)}</span>` : ''}
            </li>`).join('');
    } catch (err) {
        list.innerHTML = `<li class="print-history-empty">Could not load print history: ${escapeHtml(err.message || err)}</li>`;
    }
}

// Prints from the preview: both sides in one job (the printer flips the card), or a single side
async function printFromPreview(sides) {
    const { cancelled, notPrinted, warning } = await printCardSides(previewControlNumber, sides);
    if (cancelled) return;
    showPrintWarning(notPrinted ? 'Print cancelled or failed - nothing was recorded in the print log.' : warning);
    if (!notPrinted) loadPrintHistory(previewControlNumber);
}

printBothBtn.addEventListener('click', () => {
    if (previewCards.front && previewCards.back) {
        printFromPreview([{ side: 'front', svg: previewCards.front }, { side: 'back', svg: previewCards.back }]);
    }
});
printFrontBtn.addEventListener('click', () => {
    if (previewCards.front) printFromPreview([{ side: 'front', svg: previewCards.front }]);
});
printBackBtn.addEventListener('click', () => {
    if (previewCards.back) printFromPreview([{ side: 'back', svg: previewCards.back }]);
});

function closeCardPreview() {
    previewModal.style.display = 'none';
    resetPreviewPrinting({ front: null, back: null });
    previewControlNumber = '';
    previewFront.innerHTML = '';
    previewBack.innerHTML = '';
}

document.querySelector('#records-table tbody').addEventListener('click', e => {
    const btn = e.target.closest('.view-btn');
    if (btn && !btn.disabled) openCardPreview(btn.dataset.control, btn.dataset.name);
});

document.getElementById('previewCloseBtn').addEventListener('click', closeCardPreview);
previewModal.addEventListener('click', e => {
    if (e.target === previewModal) closeCardPreview();
});
document.addEventListener('keydown', e => {
    // Esc in the reprint question closes only that question, not the preview behind it
    if (e.key === 'Escape' && previewModal.style.display !== 'none' &&
        reprintModal.style.display === 'none' && printConfirmModal.style.display === 'none') closeCardPreview();
});

previewFolderBtn.addEventListener('click', async () => {
    const result = await ipcRenderer.invoke('open-card-folder', previewControlNumber);
    if (!result.success) {
        status.innerHTML = `<i class="fas fa-exclamation-circle"></i> <span id="status-text">${escapeHtml(result.error)}</span>`;
        status.className = 'status error';
    }
});

// ---------- Print Log tab: every card side sent to the printer ----------

const logTableBody = document.querySelector('#log-table tbody');
const logSearchInput = document.getElementById('log-search');
const logReprintsOnly = document.getElementById('logReprintsOnly');
const logRangeButtons = document.querySelectorAll('.log-range [data-range]');
let printLogEntries = [];
let logRange = 'month';

function inLogRange(entry) {
    const printed = new Date(entry.printedAt);
    const now = new Date();
    switch (logRange) {
        case 'today': return printed.toDateString() === now.toDateString();
        case 'week': return now - printed <= 7 * 24 * 60 * 60 * 1000;
        case 'month': return printed.getFullYear() === now.getFullYear() && printed.getMonth() === now.getMonth();
        default: return true;
    }
}

// Tiles count the whole period; the search box and "Only reprints" only narrow the table
function renderPrintLogStats(entries) {
    const reprints = entries.filter(e => e.isReprint);
    document.getElementById('logStatReprints').textContent = reprints.length;
    document.getElementById('logStatCards').textContent = new Set(entries.map(e => e.controlNumber)).size;

    // Most common reasons, e.g. "Lost 3 · Damaged 1" ("Lost: left in jeepney" counts as Lost)
    const reasons = {};
    reprints.forEach(e => {
        const reason = (e.reason || 'No reason').split(':')[0].trim();
        reasons[reason] = (reasons[reason] || 0) + 1;
    });
    const top = Object.entries(reasons).sort((a, b) => b[1] - a[1]).slice(0, 3).map(([r, n]) => `${r} ${n}`).join(' · ');
    document.getElementById('logStatReasons').textContent = top || 'none in this period';
}

function renderPrintLog() {
    const inPeriod = printLogEntries.filter(inLogRange);
    renderPrintLogStats(inPeriod);

    const term = logSearchInput.value.trim().toLowerCase();
    const rows = inPeriod.filter(e =>
        (!logReprintsOnly.checked || e.isReprint) &&
        (!term || [e.name, e.controlNumber, e.reason, e.printer].some(v => String(v || '').toLowerCase().includes(term)))
    );

    const table = document.getElementById('log-table');
    const empty = document.getElementById('log-empty');
    if (!rows.length) {
        table.hidden = true;
        empty.hidden = false;
        document.getElementById('log-empty-title').textContent = printLogEntries.length ? 'No prints match' : 'Nothing printed yet';
        document.getElementById('log-empty-text').textContent = printLogEntries.length
            ? 'Try another period, turn off "Only reprints", or clear the search.'
            : 'Cards appear here when they are sent to the printer.';
        logTableBody.innerHTML = '';
        return;
    }
    table.hidden = false;
    empty.hidden = true;

    const dash = '<span class="cell-empty">—</span>';
    logTableBody.innerHTML = rows.map(e => {
        const printed = new Date(e.printedAt);
        return `<tr class="${e.isReprint ? 'log-reprint' : ''}">
            <td>${escapeHtml(printed.toLocaleDateString())}<span class="cell-sub">${escapeHtml(printed.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }))}</span></td>
            <td class="cell-name">${e.name ? escapeHtml(e.name) : dash}</td>
            <td><span class="control-chip">${escapeHtml(e.controlNumber)}</span></td>
            <td><span class="card-chip ${e.side === 'back' ? 'back' : 'front'}">${e.side === 'back' ? 'Back' : 'Front'}</span></td>
            <td>${e.isReprint ? '<span class="badge badge-rehire">Reprint</span>' : '<span class="badge badge-muted">First print</span>'}</td>
            <td class="log-reason">${e.reason ? escapeHtml(e.reason) : dash}</td>
            <td>${e.printer ? escapeHtml(e.printer) : dash}</td>
            <td class="cell-actions"><button class="view-btn" data-control="${escapeHtml(e.controlNumber)}" data-name="${escapeHtml(e.name)}"><i class="fas fa-eye"></i> View</button></td>
        </tr>`;
    }).join('');
}

async function loadPrintLog() {
    try {
        const result = await ipcRenderer.invoke('get-print-log');
        if (!result.success) throw new Error(result.error);
        printLogEntries = result.entries || [];
    } catch (err) {
        console.error('Failed to load the print log:', err);
        printLogEntries = [];
    }
    renderPrintLog();
}

logRangeButtons.forEach(btn => {
    btn.addEventListener('click', () => {
        logRange = btn.dataset.range;
        logRangeButtons.forEach(b => {
            b.classList.toggle('active', b === btn);
            b.setAttribute('aria-checked', String(b === btn));
        });
        renderPrintLog();
    });
});
logReprintsOnly.addEventListener('change', renderPrintLog);
logSearchInput.addEventListener('input', renderPrintLog);

logTableBody.addEventListener('click', e => {
    const btn = e.target.closest('.view-btn');
    if (btn) openCardPreview(btn.dataset.control, btn.dataset.name);
});

function showRecordsEmpty(title, text) {
    document.getElementById('records-table').hidden = true;
    document.getElementById('records-empty-title').textContent = title;
    document.getElementById('records-empty-text').textContent = text;
    document.getElementById('records-empty').hidden = false;
}

function hideRecordsEmpty() {
    document.getElementById('records-table').hidden = false;
    document.getElementById('records-empty').hidden = true;
}

function escapeHtml(value) {
    return String(value)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function updateSortIndicators() {
    const headers = document.querySelectorAll('#records-table th[data-sort]');
    headers.forEach(th => {
        th.classList.remove('sort-asc', 'sort-desc');
        if (th.dataset.sort === recordsSortColumn) {
            th.classList.add(recordsSortDirection === 'asc' ? 'sort-asc' : 'sort-desc');
        }
    });
}

// Initialize button state on load
checkReady();

// Initialize auto-capitalize on load
setupAutoCapitalize();

// Initialize back panel name formatting
setupBackNameFormatting();