use crate::types::DataType;
use std::collections::HashSet;

pub(super) struct HeaderTerms {
    pub(super) compact: String,
    tokens: HashSet<String>,
}

impl HeaderTerms {
    fn has(&self, token: &str) -> bool {
        self.tokens.contains(token)
    }

    fn has_all(&self, tokens: &[&str]) -> bool {
        tokens.iter().all(|token| self.has(token))
    }
}

pub(super) fn terms(column_name: &str) -> HeaderTerms {
    HeaderTerms {
        compact: compact(column_name),
        tokens: tokens(column_name),
    }
}

pub(super) fn infer_secret(terms: &HeaderTerms) -> bool {
    matches!(
        terms.compact.as_str(),
        "apikey"
            | "accesstoken"
            | "authtoken"
            | "password"
            | "passwd"
            | "pwd"
            | "secret"
            | "token"
            | "privatekey"
    ) || terms.has("secret")
        || terms.has("password")
        || terms.has("passwd")
        || terms.has("pwd")
        || terms.has("token")
        || terms.has_all(&["key", "api"])
}

pub(super) fn infer_account_number(terms: &HeaderTerms) -> bool {
    matches!(
        terms.compact.as_str(),
        "accountid"
            | "accountnumber"
            | "acctid"
            | "acctnumber"
            | "customerid"
            | "userid"
            | "iban"
            | "routingnumber"
            | "cardnumber"
            | "banknumber"
    ) || terms.has("acct")
        || terms.has("iban")
        || terms.has("routing")
        || terms.has("card")
        || terms.has("pan")
        || terms.has("account") && has_identifier_token(terms)
        || terms.has_all(&["bank", "number"])
}

pub(super) fn infer_account_identifier(terms: &HeaderTerms) -> bool {
    matches!(
        terms.compact.as_str(),
        "username"
            | "userlogin"
            | "login"
            | "screenname"
            | "handle"
            | "accountname"
            | "accountusername"
    ) || terms.compact.ends_with("username")
        || terms.compact.ends_with("accountname")
        || terms.compact.ends_with("userlogin")
        || terms.compact.contains("username")
        || terms.has("username")
        || terms.has("login")
        || terms.has("handle")
        || terms.has("screenname")
        || terms.has_all(&["user", "name"])
        || terms.has_all(&["account", "name"])
}

pub(super) fn infer_numeric_id(column_name: &str) -> bool {
    let terms = terms(column_name);

    matches!(
        terms.compact.as_str(),
        "id" | "userid"
            | "usernumber"
            | "customerid"
            | "customernumber"
            | "clientid"
            | "clientnumber"
            | "accountid"
            | "accountnumber"
            | "orderid"
            | "ordernumber"
            | "code"
    ) || terms.has("id")
        || terms.has("identifier")
        || terms.has("code")
        || terms.has_all(&["account", "number"])
        || terms.has_all(&["customer", "number"])
        || terms.has_all(&["client", "number"])
        || terms.has_all(&["order", "number"])
}

pub(super) fn infer_private_date(column_name: &str) -> bool {
    let terms = terms(column_name);
    matches!(
        terms.compact.as_str(),
        "dob" | "dateofbirth" | "birthdate" | "birthday"
    ) || terms.compact.ends_with("birthdate")
        || terms.compact.ends_with("birthday")
        || terms.has("dob")
        || terms.has("dateofbirth")
        || terms.has("birthdate")
        || terms.has_all(&["date", "birth"])
}

pub(super) fn infer_user_event_date(column_name: &str) -> bool {
    let terms = terms(column_name);
    matches!(
        terms.compact.as_str(),
        "lastlogin" | "lastloginat" | "lastseen" | "lastactive" | "lastactivityat" | "loginat"
    ) || terms.compact.ends_with("lastlogin")
        || terms.compact.ends_with("lastloginat")
        || terms.compact.ends_with("lastseen")
        || terms.compact.ends_with("lastactive")
        || terms.compact.contains("lastlogin")
}

pub(super) fn infer_postal_code(column_name: &str) -> bool {
    let terms = terms(column_name);

    matches!(
        terms.compact.as_str(),
        "zip" | "zipcode" | "postalcode" | "postcode"
    ) || terms.compact.ends_with("zipcode")
        || terms.compact.ends_with("postalcode")
        || terms.compact.ends_with("postcode")
        || terms.compact.contains("zipcode")
        || terms.has("zip")
        || terms.has("zipcode")
        || terms.has("postalcode")
        || terms.has("postcode")
        || terms.has_all(&["postal", "code"])
        || terms.has_all(&["post", "code"])
}

pub(super) fn infer_phone(column_name: &str) -> bool {
    let terms = terms(column_name);

    matches!(
        terms.compact.as_str(),
        "phone"
            | "phonenumber"
            | "mobile"
            | "mobilephone"
            | "telephone"
            | "tel"
            | "cell"
            | "cellphone"
            | "homephone"
            | "workphone"
            | "businessphone"
            | "officephone"
            | "primaryphone"
            | "secondaryphone"
            | "contactphone"
            | "smsnumber"
    ) || terms.compact.ends_with("phonenumber")
        || terms.compact.contains("phonenumber")
        || terms.has("phone")
        || terms.has("phonenumber")
        || terms.has("mobile")
        || terms.has("telephone")
        || terms.has("tel")
        || terms.has("cell")
}

pub(super) fn infer_address(column_name: &str) -> bool {
    let terms = terms(column_name);

    matches!(
        terms.compact.as_str(),
        "address" | "streetaddress" | "mailingaddress"
    ) || terms.has("address")
        || terms.has("street")
}

pub(super) fn infer_tax_id(column_name: &str) -> bool {
    let terms = terms(column_name);

    matches!(
        terms.compact.as_str(),
        "ssn" | "taxid" | "taxnumber" | "ein"
    ) || terms.has("ssn")
        || terms.has("ein")
        || terms.has("tax") && (terms.has("id") || terms.has("number"))
}

pub(super) fn infer_name_type(column_name: &str) -> Option<DataType> {
    let terms = terms(column_name);

    if matches!(
        terms.compact.as_str(),
        "firstname" | "givenname" | "forename"
    ) || terms.has("firstname")
        || terms.has("forename")
        || terms.has("given")
        || terms.has_all(&["first", "name"])
    {
        return Some(DataType::FirstName);
    }

    if matches!(
        terms.compact.as_str(),
        "lastname" | "surname" | "familyname"
    ) || terms.has("lastname")
        || terms.has("surname")
        || terms.has_all(&["family", "name"])
        || terms.has_all(&["last", "name"])
    {
        return Some(DataType::LastName);
    }

    if matches!(
        terms.compact.as_str(),
        "name"
            | "fullname"
            | "displayname"
            | "legalname"
            | "personname"
            | "contactname"
            | "customername"
            | "clientname"
    ) || terms.has("fullname")
        || terms.has_all(&["display", "name"])
        || terms.has_all(&["legal", "name"])
        || terms.has_all(&["person", "name"])
        || terms.has_all(&["contact", "name"])
        || terms.has_all(&["customer", "name"])
        || terms.has_all(&["client", "name"])
        || terms.has_all(&["full", "name"])
    {
        return Some(DataType::FullName);
    }

    None
}

pub(super) fn infer_generic_name(column_name: &str) -> bool {
    compact(column_name) == "name"
}

fn has_identifier_token(terms: &HeaderTerms) -> bool {
    terms.has("id") || terms.has("identifier") || terms.has("number") || terms.has("code")
}

fn compact(column_name: &str) -> String {
    column_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn tokens(column_name: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for token in column_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        tokens.insert(token.to_ascii_lowercase());
        for subtoken in camel_case_tokens(token) {
            tokens.insert(subtoken);
        }
    }
    tokens
}

fn camel_case_tokens(token: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in token.chars() {
        if character.is_ascii_uppercase() && !current.is_empty() {
            tokens.push(current.to_ascii_lowercase());
            current.clear();
        }
        current.push(character);
    }
    if !current.is_empty() {
        tokens.push(current.to_ascii_lowercase());
    }
    tokens
}
