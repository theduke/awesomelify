use std::sync::Arc;

use cercis::prelude::*;

use crate::source::{FullReadmeRepo, FullRepoLink, Source};

use crate::server::routes::{
    repo_page::{RepoPageQuery, RepoPageView, RepoSort},
    search::PATH_SEARCH,
};

use super::HtmlError;

const SOURCE_REPO: &str = "https://github.com/theduke/awesomelify";
const FA_GITHUB: &str = "fa-brands fa-github";
const FA_STAR: &str = "fa-solid fa-star has-text-warning";

fn source_icon_class(source: &Source) -> &'static str {
    match source {
        Source::Github => FA_GITHUB,
    }
}

fn pretty_number(n: u32) -> String {
    let n = n as f64;

    if n < 1_000.0 {
        format!("{:.0}", n)
    } else {
        let ks = n / 1_000.0;
        format!("{:.0}k", ks)
    }
}

struct LinkTree {
    id: Option<String>,
    name: Option<String>,
    section: Vec<String>,

    links: Vec<FullRepoLink>,
    categories: Vec<(String, LinkTree)>,
}

impl LinkTree {
    fn name_to_id(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .map(|c| match c {
                'a'..='z' | '0'..='9' | '_' => c,
                _ => '_',
            })
            .collect()
    }

    fn new_root() -> Self {
        Self {
            id: None,
            name: None,
            section: Vec::new(),
            links: Vec::new(),
            categories: Vec::new(),
        }
    }

    fn new_child(section: &[String]) -> Self {
        let mut tree = Self::new_root();

        if let Some(name) = section.last() {
            tree.name = Some(name.to_string());

            let id = Self::name_to_id(name);
            tree.id = Some(id);

            tree.section = section.to_vec();
        }

        tree
    }

    fn category_mut(&mut self, name: &str) -> &mut LinkTree {
        if let Some(index) = self.categories.iter().position(|(n, _)| n == name) {
            return &mut self.categories[index].1;
        }

        let mut section = self.section.clone();
        section.push(name.to_string());

        let mut tree = LinkTree::new_child(&section);
        tree.name = Some(name.to_string());
        self.categories.push((name.to_string(), tree));
        &mut self.categories.last_mut().unwrap().1
    }

    fn category_mut_nested(&mut self, names: &[String]) -> &mut LinkTree {
        let mut tree = self;
        for name in names {
            tree = tree.category_mut(name);
        }
        tree
    }

    fn visit_mut<F>(&mut self, f: F)
    where
        F: Fn(&mut LinkTree) + Copy,
    {
        f(self);
        for (_, tree) in &mut self.categories {
            tree.visit_mut(f);
        }
    }

    // fn sort_categories(&mut self) {
    //     self.categories.sort_by(|(a, _), (b, _)| a.cmp(b));
    //     for (_, tree) in &mut self.categories {
    //         tree.sort_categories();
    //     }
    // }

    fn sort_links_by<F>(&mut self, f: F)
    where
        F: Fn(&FullRepoLink, &FullRepoLink) -> std::cmp::Ordering + Copy,
    {
        self.links.sort_by(|a, b| f(a, b));
        for (_, tree) in &mut self.categories {
            tree.sort_links_by(f);
        }
    }
}

fn group_links_by_category(links: &[FullRepoLink]) -> LinkTree {
    let mut root = LinkTree::new_root();

    for link in links {
        let category = root.category_mut_nested(&link.link.section);
        category.links.push(link.clone());
    }

    root.categories.sort_by(|(a, _), (b, _)| a.cmp(b));

    root
}

#[component]
fn LinkTreeIndex<'a>(tree: &'a LinkTree) -> Element {
    let content = if let (Some(name), Some(id)) = (&tree.name, &tree.id) {
        rsx! {
            div {
                class: "mb-2",
                a {
                    href: "#{id}",
                    "{name}"
                }
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        li {
            content

            ul {
                for (_name, category) in tree.categories.iter() {
                    LinkTreeIndex {
                        tree: category,
                    }
                }
            }
        }
    }
}

struct UnescapedHtml(String);

impl cercis::html::Render for UnescapedHtml {
    fn render(&self) -> String {
        self.0.clone()
    }
}

#[component]
fn LinkTreeRoot<'a>(tree: &'a LinkTree) -> Element {
    // JS for toggling the index.
    let script = UnescapedHtml(
        r#"
(function() {
    const button = document.querySelector('#index-toggle');
    const content = document.querySelector('#index-content');


    button.addEventListener('click', function() {
        const isHidden = content.classList.contains('is-hidden');
        if (isHidden) {
            content.classList.remove('is-hidden');
            button.innerHTML = 'Hide';
        } else {
            content.classList.add('is-hidden');
            button.innerHTML = 'Show';
        }
    });
})()
"#
        .to_string(),
    );

    rsx! {
        div {
            div {
                class: "box",
                div {
                    h3 {
                        class: "title is-3",
                        "Index"

                        button {
                            id: "index-toggle",
                            class: "button is-small ml-2",
                            style: "margin-top: 5px",
                            "Hide"
                        }
                    }

                }

                div {
                    id: "index-content",
                    ul {
                        class: "content",

                        LinkTreeIndex {
                            tree: tree,
                        }
                    }
                }

                script {
                    script
                }
            }

            LinkTreeView {
                tree: tree,
            }
        }
    }
}

#[component]
fn LinkTreeView<'a>(tree: &'a LinkTree) -> Element {
    let id = tree.id.as_deref().unwrap_or_default();

    let heading = if !tree.section.is_empty() {
        let full_name = tree.section.join(" > ");
        rsx! {
            h4 {
                class: "title is-4",
                "{full_name}"
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        div {
            id: "{id}",

            if !tree.links.is_empty() {
                div {
                    class: "box mb-4",

                    heading

                    LinksTable {
                        links: &tree.links,
                        show_category: false,
                    }
                }
            }

            for (_name, category) in tree.categories.iter() {
                LinkTreeView {
                    tree: category,
                }
            }
        }
    }
}

#[component]
fn AddonField<'a>(children: Element<'a>) -> Element {
    rsx! {
        div {
            class: "field has-addons",

            children
        }
    }
}

#[component]
fn AddonFieldButton<'a>(url: String, icon: &'a str, name: &'a str, is_active: bool) -> Element {
    let class = if *is_active {
        "button is-info"
    } else {
        "button"
    };

    rsx! {
        p {
            class: "control",

            a {
                class: "{class}",
                "hx-boost": "true",
                href: "{url}",
                title: "Sort by {name}",
                span {
                    class: "icon",
                    i {
                        class: "{icon}",
                    }
                }
                span {
                    "{name}"
                }
            }
        }
    }
}

#[component]
pub fn ReadmeRepoPage<'a>(
    repo: &'a FullReadmeRepo,
    tree: &'a LinkTree,
    query: RepoPageQuery,
) -> Element {
    let details = &repo.repo.details;
    let name = format!("{}/{}", details.ident.owner, details.ident.repo);

    let missing_repos = repo.missing_links_count();
    let repo_mismatch_warning = if missing_repos > 0 {
        rsx! {
            div {
                class: "notification is-warning",
                "Some repos have not been loaded yet, probably due to API rate limiting. Check back later."
                " ({missing_repos} missing)"
            }
        }
    } else {
        rsx! {}
    };

    let icon = source_icon_class(&repo.repo.details.ident.source);

    let view = query.view.unwrap_or(RepoPageView::TablePerCategory);
    let sort = query.sort.unwrap_or(RepoSort::Stars);

    let link_view_single_table = {
        let mut query = query.clone();
        query.view = Some(RepoPageView::SingleTable);
        query.to_query()
    };
    let link_view_multi_table = {
        let mut query = query.clone();
        query.view = Some(RepoPageView::TablePerCategory);
        query.to_query()
    };

    let link_sort_title = {
        let mut query = query.clone();
        query.sort = Some(RepoSort::Title);
        query.to_query()
    };
    let link_sort_stars = {
        let mut query = query.clone();
        query.sort = Some(RepoSort::Stars);
        query.to_query()
    };
    let link_sort_updated = {
        let mut query = query.clone();
        query.sort = Some(RepoSort::Updated);
        query.to_query()
    };

    let view_selector = rsx! {
        div {
            class: "is-flex",
            style: "gap: 2rem",

            div {
                b {
                    "View: "
                }
            }

            div {
                AddonField {
                    AddonFieldButton {
                        url: link_view_single_table,
                        icon: "fa-solid fa-table",
                        is_active: view == RepoPageView::SingleTable,
                        name: "Single table",
                    }

                    AddonFieldButton {
                        url: link_view_multi_table,
                        icon: "fa-solid fa-table-list",
                        is_active: view == RepoPageView::TablePerCategory,
                        name: "Table per category",
                    }

                    AddonFieldButton {
                        // TODO: implement...
                        url: "#".to_string(),
                        is_active: view == RepoPageView::List,
                        icon: "fa-solid fa-list",
                        name: "List",
                    }
                }
        }
      }
    };

    let sort_selector = rsx! {
        div {
            class: "is-flex",
            style: "gap: 2rem",

            div {
                b {
                    "Sort: "
                }
            }

            div {
                AddonField {
                    AddonFieldButton {
                        url: link_sort_title,
                        icon: "fa-solid fa-sort-alpha-up",
                        name: "Title",
                        is_active: sort == RepoSort::Title,
                    }

                    AddonFieldButton {
                        url: link_sort_stars,
                        icon: "fa-solid fa-star",
                        name: "Stars",
                        is_active: sort == RepoSort::Stars,
                    }

                    AddonFieldButton {
                        url: link_sort_updated,
                        is_active: sort == RepoSort::Updated,
                        icon: "fa-solid fa-clock",
                        name: "Updated",
                    }
                }
            }
        }
    };

    let controls = rsx! {
        div {
            class: "is-flex mb-4 box is-flex-wrap-wrap",
            style: "gap: 2rem",

            view_selector

            sort_selector
        }
    };

    let header = rsx! {
        h1 {
            class: "title is-1",
            i {
                class: "{icon}",
            }
            "  "
            a {
                href: "{details.ident.url()}",
                target: "_blank",
                class: "has-text-black",
                style: "text-decoration: none",
                "{name}"
            }
        }


        div {
            class: "box",


            div {
                class: "is-flex is-justify-content-space-between",

                p {
                    class: "has-text-centered",

                    "{details.description.as_deref().unwrap_or_default()}"
                }

                div {
                    class: "buttons",

                    a {
                        class: "button is-medium",
                        href: "{details.ident.url()}",
                        target: "_blank",

                        span {
                            class: "icon",
                            i {
                                class: "fa-brands fa-github pr-1",
                            }
                        }

                        span {
                            "{pretty_number(details.stargazer_count)} stars"
                        }
                    }

                    button {
                        class: "button is-medium",
                        span {
                            class: "icon",
                            i {
                                class: "fa-solid fa-list",
                            }
                        }
                        span {
                            "{repo.repo.repo_links.len()} repos"
                        }
                    }
                }
            }
        }

        repo_mismatch_warning
    };

    let content = match view {
        RepoPageView::SingleTable => {
            // Prevent duplicates.

            rsx! {
                div {
                    class: "box",

                    LinksTable {
                        links: &repo.links,
                        show_category: true,
                    }
                }
            }
        }
        RepoPageView::TablePerCategory => {
            rsx! {
                LinkTreeRoot {
                    tree: &tree,
                }
            }
        }
        RepoPageView::List => todo!(),
    };

    rsx! {
        div {
            header

            controls

            content
        }
    }
}

#[component]
fn LinksTable<'a>(links: &'a [FullRepoLink], show_category: bool) -> Element {
    rsx! {
        table {
            class: "table",
            style: "width: 100%",
            thead {
                tr {
                    th {
                        "Repo"
                    }
                    th {
                        "Description"
                    }
                    th {
                        i {
                            class: "{FA_STAR}",
                            title: "Star count"
                        }
                    }
                    th {
                        "Updated"
                    }
                    th {
                        "Lang"
                    }

                    if *show_category {
                        th {
                            "Category"
                        }
                    } else {
                    }
                }
            }
            tbody {
                for link in links.iter() {
                    tr {
                        td {
                            a {
                                href: "{link.link.ident.url()}",
                                target: "_blank",
                                "{link.link.ident.owner}/{link.link.ident.repo}"
                            }
                        }
                        td {
                            "{link.details.description.as_deref().unwrap_or_default()}"
                        }
                        td {
                            "{pretty_number(link.details.stargazer_count)}"
                        }
                        td {
                            "{link.details.last_activity_relative_time().unwrap_or_default()}"
                        }
                        td {
                            "{link.details.primary_language.as_deref().unwrap_or_default()}"
                        }

                        if *show_category {
                            td {
                                "{link.link.section.join(\">\")}"
                            }
                        } else {
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn PageLayout<'a>(title: &'a str, children: Element<'a>) -> Element {
    rsx! {
        html {
            head {
                link {
                    rel: "stylesheet",
                    href: "https://cdn.jsdelivr.net/npm/bulma@1.0.1/css/bulma.min.css",
                }
                link {
                    rel: "stylesheet",
                    href: "/static/style.css",
                }
                link {
                    rel: "stylesheet",
                    href: "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.6.0/css/all.min.css",
                    integrity: "sha512-Kc323vGBEqzTmouAECnVceyQqyqdsSiqLQISBL29aUW4U/M7pSPA/gEUZQqv1cwx4OnYxTxve5UMg5GT6L4JJg==",
                    crossorigin: "anonymous",
                    referrerpolicy: "no-referrer",
                }

                script {
                    src: "https://unpkg.com/htmx.org@2.0.1",
                    integrity: "sha384-QWGpdj554B4ETpJJC9z+ZHJcA/i59TyjxEPXiiUgN2WmTyV5OEZWCD6gQhgkdpB/",
                    crossorigin: "anonymous",
                }

                title {
                    "{title}"
                }
            }

            body {
                NavBar {}

                section {
                    class: "section",
                    children
                }

                footer {
                    class: "footer",
                    div {
                        class: "content has-text-centered",
                        p {
                            "awesomelify"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NavBar() -> Element {
    rsx! {
        nav {
            class: "navbar",
            role: "navigation",
            aria_label: "main navigation",

            div {
                class: "navbar-brand",

                a {
                    class: "navbar-item",
                    "awesomelify"
                }
            }

            div {
                class: "navbar-menu",

                div {
                    class: "navbar-start",
                    a {
                        class: "navbar-item",
                        href: "/",
                        "Home"
                    }
                }

                div {
                    class: "navbar-end",

                    div {
                        class: "navbar-item",

                        div {
                            class: "buttons",

                            a {
                                class: "button",
                                target: "_blank",
                                href: "{SOURCE_REPO}",

                                span {
                                    class: "icon",
                                    i {
                                        class: "{FA_GITHUB}",
                                    }
                                }

                                span {
                                    "Source"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn render_repo_page(mut repo: FullReadmeRepo, query: RepoPageQuery) -> String {
    let mut tree = group_links_by_category(&repo.links);

    // Filter out duplicates.
    {
        let mut seen = std::collections::HashSet::new();

        repo.links
            .retain(|link| seen.insert(link.link.ident.clone()));

        tree.visit_mut(|tree| {
            let mut seen = std::collections::HashSet::new();

            tree.links
                .retain(|link| seen.insert(link.link.ident.clone()));
        })
    }

    let sort = query.sort.unwrap_or(RepoSort::Stars);
    match sort {
        RepoSort::Title => {
            repo.links.sort_by(|a, b| a.link.ident.cmp(&b.link.ident));
            tree.sort_links_by(|a, b| a.link.ident.cmp(&b.link.ident))
        }
        RepoSort::Stars => {
            repo.links
                .sort_by(|a, b| b.details.stargazer_count.cmp(&a.details.stargazer_count));
            tree.sort_links_by(|a, b| b.details.stargazer_count.cmp(&a.details.stargazer_count))
        }
        RepoSort::Updated => {
            repo.links
                .sort_by(|a, b| b.details.last_activity().cmp(&a.details.last_activity()));
            tree.sort_links_by(|a, b| b.details.last_activity().cmp(&a.details.last_activity()))
        }
    };

    

    rsx! {
        PageLayout {
            title: &repo.repo.details.ident.repo,
            ReadmeRepoPage {
                repo: &repo,
                tree: &tree,
                query: query,
            }
        }
    }
    .render()
}

#[component]
fn SearchBar() -> Element {
    rsx! {
        form {
            method: "GET",
            action: "{PATH_SEARCH}",

            div {
                class: "field has-addons",

                p {
                    class: "control",
                    button {
                        r#type: "button",
                        class: "button",
                        "Repo"
                    }
                }

                p {
                    class: "control is-expanded",
                    input {
                        name: "q",
                        class: "input",
                        r#type: "text",
                        title: "Repository URL",
                        "aria-label": "Repository URL",
                        placeholder: "github.com/org/repo",
                        "hx-get": "{PATH_SEARCH}",
                        "hx-target": "#search-results",
                        "hx-trigger":"keyup changed delay:500ms, search",
                        "hx-indicator": "#search-spinner",
                    }
                }

                p {
                    class: "control",
                    button {
                        class: "button",
                        r#type: "submit",

                        span {
                            class: "icon",
                            i {
                                class: "fa-solid fa-search",
                                "aria-label": "Open repository",
                            }
                        }
                    }
                }
            }

            p {
                class: "mb-2",
                "Open a repo. Note: this only makes sense with awesome- style link collections! "
                "Only Github is supported for now..."
            }

            div {
                id: "search-results",

                div {
                    class: "htmx-indicator",
                    id: "search-spinner",
                    Spinner {}
                }
            }
        }
    }
}

#[component]
fn Spinner() -> Element {
    rsx! {
        button {
            class: "button is-loading",
        }
    }
}

#[component]
fn Homepage(popular_repos: Vec<Arc<FullReadmeRepo>>) -> Element {
    rsx! {

        div {
            class: "is-flex is-justify-content-center is-align-items-center mb-4",

            div {
                class: "box has-text-centered",
                style: "font-size: 1.35rem",

                p {
                    b { "awesomelify" }
                    " lets you browse awesome- lists on Github in a more user-friendly way."
                }

                p {
                    "It fetches the READMEs of awesome lists, extracts linked repositories and shows the repositories with structured metadata, including:"
                }

                p {
                    b { "current star count"}
                    ","
                    b { "last commit date" }
                    ","
                    b { "issue and fork count" }
                    ","
                    b { "programming language" }
                    ", ..."
                }
            }

        }

        div {
            class: "is-flex is-justify-content-center is-align-items-center",

            div {
                class: "box",
                style: "max-width: 500px",

                SearchBar {}

            }
        }

        hr {}

        div {
            h4 {
                class: "title is-4 has-text-centered",
                "Popular lists"
            }

            div {
                class: "columns is-multiline",

                for repo in popular_repos {
                    RepoLinkBox {
                        repo: &repo,
                    }
                }
            }
        }
    }
}

#[component]
fn RepoLinkBox<'a>(repo: &'a FullReadmeRepo) -> Element {
    let details = &repo.repo.details;
    let ident = &details.ident;

    let link = format!("/repo/{}/{}/{}", ident.source, ident.owner, ident.repo);

    let icon = source_icon_class(&ident.source);

    rsx! {
        div {
            class: "column is-one-third",
            div {
                class: "box is-flex is-flex-direction-column",
                style: "gap: 0.7rem;",

                div {
                    a {
                        href: "{link}",
                        class: "has-text-black is-underlined",
                        style: "font-size: 1.4rem;",

                        span {
                            class: "icon",
                            i {
                                class: "{icon}",
                            }
                        }

                        span {
                            class: "pl-3",
                            "{ident.name()}"
                        }

                    }
                }

                p {
                    "{details.description.as_deref().unwrap_or_default()}"
                }

                div {
                    class: "is-flex",
                    style: "gap: 0.7rem",

                    button {
                        class: "button is-small is-outlined has-text-black",
                        span {
                            class: "icon",
                            i {
                                class: "fa-solid fa-list",
                            }
                        }
                        span {
                            "{repo.links.len()} repos"
                        }
                    }

                    button {
                        class: "button is-small is-outlined has-text-black",
                        span {
                            class: "icon",
                            i {
                                class: "{FA_STAR}",
                            }
                        }
                        span {
                            "{pretty_number(details.stargazer_count)} stars"
                        }
                    }
                }
            }
        }
    }
}

pub fn render_homepage(popular_repos: Vec<Arc<FullReadmeRepo>>) -> String {
    let output = rsx! {
        PageLayout {
            title: "awesomelify - awesome- Link List Viewer",
            Homepage {
                popular_repos: popular_repos,
            }
        }
    };

    output.render()
}

#[component]
fn HtmlErrorView<'a>(error: &'a HtmlError) -> Element {
    let details = if let Some(err) = &error.source {
        let content = UnescapedHtml(format!("{:#?}", err));

        rsx! {
            hr {}
            pre {
                content
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        p {
            class: "notification is-danger",

            "{error.message}"

            details
        }
    }
}

pub fn render_html_error_standalone(error: &HtmlError) -> String {
    let output = rsx! {
        HtmlErrorView {
            error: error,
        }
    };
    output.render()
}

pub fn render_html_error_page(error: &HtmlError) -> String {
    let output = rsx! {
        PageLayout {
            title: "Error",
            HtmlErrorView {
                error: error,
            }
        }
    };
    output.render()
}
