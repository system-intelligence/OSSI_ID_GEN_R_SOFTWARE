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
            default: return Promise.reject(new Error(`Unknown channel: ${channel}`));
        }
    }
};
const { looksLikeMobile, isValidMobile } = window.OssiPhone;

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
            [preview, sigPreview].forEach(img => img.style.display = 'none');
            document.querySelectorAll('.placeholder-text').forEach(el => el.style.display = 'block');
            positionOtherInput.style.display = 'none';
            
            idPicBase64 = null;
            sigBase64 = null;
            dataUpdated = false;
            generatedControlNumber = '';
            issuedControlNumber = '';

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

function checkReady() {
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
        const contactValue = backContactInput.value.trim();
        if (contactValue && looksLikeMobile(contactValue) && !isValidMobile(contactValue)) {
            status.innerHTML = '<i class="fas fa-exclamation-circle"></i> <span id="status-text">Contact number is not a valid mobile number - it must be 11 digits, like 0917 123 4567</span>';
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
            contact: backContactInput.value.trim() || 'PHONE NUMBER',
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
        const records = await ipcRenderer.invoke('get-all-records');
        
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
            
            if (recordsSortColumn === 'no' || recordsSortColumn === 'isRehire') {
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
                <td class="emergency-cell cell-name">${cell(r.emergencyName)}</td>
                <td class="emergency-cell">${r.relationship ? `<span class="badge badge-relationship">${escapeHtml(r.relationship)}</span>` : dash}</td>
                <td class="emergency-cell">${cell(r.contact)}</td>
                <td class="emergency-cell cell-address">${address || dash}</td>
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