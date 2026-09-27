function value(object, key) {
    const result = object[key]();
    if (result === null || result === undefined) return '';
    if (result instanceof Date) return result.toISOString();
    return String(result);
}

function each(collection, callback) {
    const results = [];
    for (let index = 0, count = collection.length; index < count; index++) results.push(callback(collection[index]));
    return results;
}

function contacts() {
    return each(Application('Contacts').people, function (person) {
        const fields = [];
        function add(id, group, label, text) {
            if (text.trim()) fields.push({id: id, group: group, label: label, value: text});
        }
        [
            ['name', 'name', 'Name'], ['firstName', 'name', 'First name'],
            ['middleName', 'name', 'Middle name'], ['lastName', 'name', 'Last name'],
            ['nickname', 'name', 'Nickname'], ['maidenName', 'name', 'Maiden name'],
            ['title', 'name', 'Title'], ['suffix', 'name', 'Suffix'],
            ['phoneticFirstName', 'name', 'Phonetic first name'],
            ['phoneticMiddleName', 'name', 'Phonetic middle name'],
            ['phoneticLastName', 'name', 'Phonetic last name'],
            ['organization', 'work', 'Organization'], ['department', 'work', 'Department'],
            ['jobTitle', 'work', 'Job title'], ['homePage', 'link', 'Home page'],
            ['birthDate', 'other', 'Birthday'], ['note', 'other', 'Notes'],
            ['creationDate', 'other', 'Created'], ['modificationDate', 'other', 'Modified']
        ].forEach(function (field) { add(field[0], field[1], field[2], value(person, field[0])); });
        [
            ['phones', 'phone', 'Phone'], ['emails', 'email', 'Email'],
            ['urls', 'link', 'URL'], ['customDates', 'other', 'Date'],
            ['relatedNames', 'other', 'Related name']
        ].forEach(function (field) {
            each(person[field[0]], function (item) {
                add(value(item, 'id'), field[1], value(item, 'label') || field[2], value(item, 'value'));
            });
        });
        each(person.addresses, function (item) {
            const address = ['street', 'city', 'state', 'zip', 'country', 'countryCode']
                .map(function (key) { return value(item, key); }).filter(Boolean).join(', ');
            add(value(item, 'id'), 'address', value(item, 'label') || 'Address', address);
        });
        each(person.instantMessages, function (item) {
            add(value(item, 'id'), 'link', value(item, 'label') || 'Instant message',
                [value(item, 'serviceName'), value(item, 'userName')].filter(Boolean).join(': '));
        });
        each(person.socialProfiles, function (item) {
            add(value(item, 'id'), 'link', value(item, 'serviceName') || 'Social profile',
                [value(item, 'userName'), value(item, 'userIdentifier'), value(item, 'url')].filter(Boolean).join(' '));
        });
        return {id: value(person, 'id'), name: value(person, 'name'), fields: fields};
    });
}

function tabs(browser) {
    const app = Application(browser === 'chrome' ? 'Google Chrome' : 'Arc');
    if (!app.running()) throw new Error(browser + ' is not running');
    const rows = [], seen = {};
    function append(window, collection, space) {
        const windowId = value(window, 'id');
        const ids = collection.id(), titles = collection.title(), urls = collection.url();
        const locations = browser === 'arc' ? collection.location() : [];
        for (let index = 0; index < ids.length; index++) {
            const key = windowId + ':' + String(ids[index]);
            if (seen[key]) continue;
            seen[key] = true;
            rows.push({id: key, window: windowId, tab: String(ids[index]),
                title: titles[index], url: urls[index], space: space, location: locations[index] || ''});
        }
    }
    each(app.windows, function (window) {
        if (browser === 'arc') each(window.spaces, function (space) {
            append(window, space.tabs, {id:value(space, 'id'), title:value(space, 'title')});
        });
        append(window, window.tabs, null);
    });
    return rows;
}

function focus(browser, item) {
    const app = Application(browser === 'chrome' ? 'Google Chrome' : 'Arc');
    if (!app.running()) throw new Error(browser + ' is not running');
    const windows = app.windows;
    for (let w = 0; w < windows.length; w++) {
        const window = windows[w];
        if (value(window, 'id') !== item.window) continue;
        let collection = window.tabs;
        if (browser === 'arc' && item.space) {
            const spaces = window.spaces;
            for (let s = 0; s < spaces.length; s++) {
                if (value(spaces[s], 'id') === item.space.id) {
                    collection = spaces[s].tabs;
                    for (let t = 0; t < collection.length; t++) if (value(collection[t], 'id') === item.tab) {
                        spaces[s].focus(); collection[t].select(); window.index = 1; app.activate(); return true;
                    }
                }
            }
        }
        for (let t = 0; t < collection.length; t++) if (value(collection[t], 'id') === item.tab) {
            if (browser === 'arc') collection[t].select();
            else window.activeTabIndex = t + 1;
            window.index = 1; app.activate(); return true;
        }
    }
    throw new Error('Tab closed or moved. Press Alt-R to refresh.');
}

function run(argv) {
    const request = JSON.parse(argv[0]);
    if (request.action === 'contacts') return JSON.stringify(contacts());
    if (request.action === 'tabs') return JSON.stringify(tabs(request.browser));
    if (request.action === 'focus') return JSON.stringify(focus(request.browser, request.item));
    throw new Error('Unknown macOS action');
}
