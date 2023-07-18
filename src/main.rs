use scraper::{Html, Selector};
use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use std::fs;
use serde_json::Number;
use regex::Regex;
use chrono::NaiveDate;
use reqwest::Client;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct Product {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    price: Option<Number>,
    image_url: String,
    capacity_mb: u32,
    colour: String,
    availability_text: String,
    is_available: bool,
    shipping_text: String,
    shipping_date: String,
}

fn standard_date_format(text: &str) -> Option<String> {
    let re = Regex::new(r"(\d{1,2}) (\w+) (\d{4})|\d{4}-\d{2}-\d{2}|\s*(\d+)(?:[a-z]{2})?\s+(?:of\s+)?([a-z]{3})\s+(\d{4})|tomorrow").unwrap();

    let dates: Vec<String> = re
        .captures_iter(text)
        .flat_map(|caps| {
            caps.iter()
                .skip(1)
                .filter_map(|m| m.map(|m| m.as_str().to_string()))
                .collect::<Vec<_>>()
        })
        .collect();

    if let Some(date_str) = dates.first() {
        let date = NaiveDate::parse_from_str(&date_str, "%d %B %Y").or_else(|_| NaiveDate::parse_from_str(&date_str, "%Y-%m-%d"));

        if let Ok(date) = date {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }

    None
}

#[tokio::main]
async fn main() {
    let base_url = "https://www.magpiehq.com/developer-challenge/smartphones";
    let mut products: HashSet<Product> = HashSet::new();
    let mut page_number = 1;

    let client = Client::new();

    loop {
        let url = format!("{}/{}", base_url, page_number);
        let document = fetch_document(&client, &url).await;
        let product_selector = Selector::parse(".product").unwrap();
        let product_elements = document.select(&product_selector).collect::<Vec<_>>();

        if product_elements.is_empty() {
            break; // No more products found, exit the loop
        }

        for product_element in product_elements {
            let title_element = product_element.select(&Selector::parse(".text-blue-600").unwrap()).next();
            let price_element = product_element.select(&Selector::parse(".text-lg").unwrap()).next();
            let capacity_element = product_element.select(&Selector::parse(".product-capacity").unwrap()).next();
            let image_element = product_element.select(&Selector::parse("img").unwrap()).next();
            let availability_element = product_element.select(&Selector::parse(".bg-white > div").unwrap()).nth(2);
            let shipping_element = product_element.select(&Selector::parse(".bg-white > div").unwrap()).last();

            if let (Some(title), Some(price), Some(capacity), Some(image), Some(availability), Some(shipping)) = (
                title_element,
                price_element,
                capacity_element,
                image_element,
                availability_element,
                shipping_element,
            ) {
                let title_text = title.inner_html();
                let price_text = price.inner_html().replace(|c: char| !c.is_digit(10) && c != '.', "");
                let capacity_text = capacity.inner_html();
                let image_url = image.value().attr("src").map(|url| url.replace("..", base_url));
                let availability_text = availability.inner_html();
                let shipping_text = shipping.inner_html();
                let colour_elements = product_element.select(&Selector::parse(".flex .px-2 > span").unwrap()).collect::<Vec<_>>();

                let colours = colour_elements
                    .iter()
                    .filter_map(|span| span.value().attr("data-colour"))
                    .collect::<Vec<_>>();

                if !colours.is_empty() {
                    let options = colours.len();

                    for (i, colour) in colours.into_iter().enumerate() {
                        let shipping_date = if !shipping_text.contains("Availability: Out of Stock") {
                            standard_date_format(&shipping_text)
                        } else {
                            None
                        };

                        let product = Product {
                            title: title_text.clone(),
                            price: if price_text.is_empty() { None } else { Some(Number::from_f64(price_text.parse::<f64>().unwrap()).unwrap()) },
                            image_url: image_url.clone().unwrap_or_default(),
                            capacity_mb: capacity_text.parse::<u32>().unwrap() * 1000,
                            colour: colour.to_string(),
                            availability_text: availability_text.clone(),
                            is_available: availability_text.contains("Availability: In Stock"),
                            shipping_text: shipping_text.clone(),
                            shipping_date: shipping_date.unwrap_or_default(),
                        };

                        products.insert(product);

                        if i == options - 1 {
                            break;
                        }
                    }
                }
            }
        }

        page_number += 1;
    }

    let unique_products: Vec<Product> = products.into_iter().collect();
    fs::write("output.json", serde_json::to_string(&unique_products).unwrap()).unwrap();
}

async fn fetch_document(client: &Client, url: &str) -> Html {
    let body = client.get(url).send().await.unwrap().text().await.unwrap();
    Html::parse_document(&body)
}
