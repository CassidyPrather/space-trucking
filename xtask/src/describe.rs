//! What a mesh looks like, in English, written by something that saw it.
//!
//! **The describer is hybrid on purpose, and the numbers are the half
//! that is true.** A vision model shown a picture of a crate writes a
//! confident sentence about a crate; it cannot tell you that the crate is
//! 412 triangles or 70 cm across, and if asked it will guess. So the
//! facts come from [`crate::preview`], which measured them with the file
//! open, and the model is asked for exactly the half a picture can
//! answer: shape, markings, colour, wear, and what the thing appears to
//! be for. The catalogue keeps them in separate fields for the same
//! reason.
//!
//! **And the model is told what it is looking at.** That is the whole
//! difference between a catalogue worth searching and four hundred lines
//! reading "a low-poly 3D model of a tree". The pack's own name for the
//! asset is in the prompt, together with its measurements, and the
//! instruction is to write what the picture says that the name does not —
//! so a describer given `SM_Tree_Pine_04` writes about *this* pine rather
//! than announcing that it is a tree. The guard at the bottom of this
//! file is the one that keeps that true.
//!
//! ## Three describers, and what each one costs
//!
//! | | |
//! | --- | --- |
//! | `$ART_DESCRIBER` | any program run as `<program> <prompt.txt> <picture.png>`, printing the description on standard output |
//! | `$OPENROUTER_API_KEY` | a hosted vision model, reached with `curl`. The default, and the one that costs money |
//! | neither | the measurements, written out as a sentence and labelled as such |
//!
//! The third is not a failure mode to be embarrassed about — a catalogue
//! of triangle counts and sizes is worth having and answers half the
//! questions people ask of one. What it must never do is *look* like the
//! second, which is why every entry records what wrote it.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cache::Cache;
use crate::dex;
use crate::fsx;
use crate::json::{self, Json};
use crate::preview::Look;

/// The model asked when nothing says otherwise: cheap, hosted, and it
/// reads pictures. `$ART_DESCRIBER_MODEL` or `--model` names another —
/// any `OpenRouter` slug that takes an image.
pub const MODEL: &str = "deepseek/deepseek-v4-flash-vision-exp";

/// Where the request goes. Overridable because the body is ordinary
/// OpenAI-shaped chat completion and several things speak it, including
/// whatever somebody is running on their own machine.
const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

/// **What the describer is told it is looking at.**
///
/// Every field here is something the picture cannot say and the file name
/// can, which is the whole reason it is passed along.
pub struct Subject<'a> {
    /// The file's own stem, as the pack spells it: `SM_Prop_Crate_01`.
    pub name: &'a str,
    /// What the store calls the pack it came out of.
    pub pack: &'a str,
}

pub enum Describer {
    /// `$ART_DESCRIBER`, run as `<program> <prompt.txt> <picture.png>`.
    Program(PathBuf),
    /// A hosted vision model, reached with `curl`.
    Hosted { model: String, key: String },
    /// Nothing to look with. The measurements, said out loud.
    Measurements,
}

impl Describer {
    /// What to say about it in a report line, and what goes in the
    /// catalogue's `described_by` field. A sentence is worth what the
    /// thing that wrote it saw, so this is never abbreviated away.
    pub fn describe(&self) -> String {
        match self {
            Self::Program(path) => format!(
                "$ART_DESCRIBER {}",
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned()
                )
            ),
            Self::Hosted { model, .. } => model.clone(),
            Self::Measurements => dex::UNDESCRIBED.to_owned(),
        }
    }

    /// One line for the run's own report: which describer, and — where it
    /// matters — why it is that one rather than the one somebody expected.
    pub fn announce(&self) -> String {
        match self {
            Self::Program(_) => format!("art: describing with {}", self.describe()),
            Self::Hosted { model, .. } => format!("art: describing with {model}, one picture each"),
            Self::Measurements => String::from(
                "art: no $OPENROUTER_API_KEY and no $ART_DESCRIBER, so every entry will carry\n     \
                 its measurements and say so. The pictures are still rendered and the counts\n     \
                 are still true; what is missing is the sentence about what it looks like.",
            ),
        }
    }

    /// **A description of one mesh, or a complaint about why there is
    /// none.**
    ///
    /// The picture is a path rather than bytes because two of the three
    /// describers hand it straight to somebody else, and the third is the
    /// one that has to read it.
    pub fn say(
        &self,
        cache: &Cache,
        subject: &Subject<'_>,
        look: &Look,
        picture: &Path,
        digest: &str,
    ) -> Result<String, String> {
        let said = match self {
            Self::Measurements => measured(look),
            Self::Program(program) => {
                let asked = cache.dex_file(digest, "prompt.txt");
                fsx::write(&asked, &prompt(subject, look))?;
                let output = Command::new(program)
                    .arg(&asked)
                    .arg(picture)
                    .output()
                    .map_err(|err| format!("cannot run {}: {err}", self.describe()))?;
                if !output.status.success() {
                    return Err(format!(
                        "{} said nothing about {}\n{}",
                        self.describe(),
                        subject.name,
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                String::from_utf8_lossy(&output.stdout).into_owned()
            }
            Self::Hosted { model, key } => ask(cache, model, key, subject, look, picture, digest)?,
        };
        let sentence = dex::sentence(&said);
        if sentence.is_empty() {
            return Err(format!(
                "{} answered about {} with nothing at all",
                self.describe(),
                subject.name
            ));
        }
        Ok(sentence)
    }
}

/// **Find a describer.** Never an error: a machine with no key and no
/// program still gets a catalogue, one that says what it is.
pub fn find(model: Option<String>) -> Describer {
    if let Some(program) = std::env::var_os("ART_DESCRIBER") {
        return Describer::Program(PathBuf::from(program));
    }
    std::env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .map_or(Describer::Measurements, |key| Describer::Hosted {
            model: model
                .or_else(|| std::env::var("ART_DESCRIBER_MODEL").ok())
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| MODEL.to_owned()),
            key,
        })
}

/// **The prompt, which is most of what this module is.**
///
/// Three parts, and each one is there because of what happens without it.
///
/// The **name** is first, because a description written without it is a
/// description of the category: shown a picture of a pine tree and asked
/// what it is, a model says it is a low-poly pine tree, which is a
/// sentence the file name already contained. Told it is looking at
/// `SM_Tree_Pine_04`, the same model spends its sentence on what
/// distinguishes this pine from the eleven others in the pack — which is
/// the question somebody browsing a catalogue actually has.
///
/// The **measurements** are next, so the model does not invent them. A
/// vision model asked how big a crate is will answer, plausibly and
/// wrongly; given the number it will use it or leave it alone.
///
/// The **instruction** is last and is mostly a list of things not to
/// write. "Do not begin with 'This is'" and "do not repeat the name" are
/// worth their characters: both are what an unprompted answer opens with,
/// and both spend a third of a three-sentence budget saying what the
/// reader is already looking at.
pub fn prompt(subject: &Subject<'_>, look: &Look) -> String {
    let size = look.size();
    let textures = if look.images.is_empty() {
        String::from("no texture reached it, so its colours here are not the pack's")
    } else {
        format!("textured from {}", look.images.join(", "))
    };
    format!(
        "The four views are one 3D model from the game-art pack \"{pack}\". The pack's own \
         name for it is \"{name}\", which reads as \"{readable}\".\n\
         \n\
         Measured from the file, so you do not have to guess: {x:.2} x {y:.2} x {z:.2} in the \
         file's own units (metres, for this pack), {triangles} triangles across {meshes} \
         and {materials}, {textures}.\n\
         \n\
         Write the catalogue entry for it: one to three sentences, at most {longest} \
         characters, plain English, no markdown, no bullet points, no preamble.\n\
         \n\
         The reader already knows the name and has the measurements beside your sentence, so \
         do not begin with \"This is\", do not repeat the name, and do not restate the size or \
         the triangle count. Write what the picture says and the name does not: the shape and \
         proportions, what is built onto it, its markings, its colours and materials, its \
         condition, what it looks like it is for, and anything that would make somebody \
         choose this one over another asset with a name like it. If what you can see \
         disagrees with the name, describe what you can actually see.",
        pack = subject.pack,
        name = subject.name,
        readable = readable(subject.name),
        x = size[0],
        y = size[1],
        z = size[2],
        triangles = look.triangles,
        meshes = count(look.meshes, "mesh", "meshes"),
        materials = count(look.materials, "material", "materials"),
        textures = textures,
        longest = dex::LONGEST,
    )
}

/// A file name as a person would read it aloud. `SM_Prop_Crate_01`
/// becomes `Prop Crate 01`: the underscores go, and so does the leading
/// `SM`/`SK`/`SM_` an exporter put there to say "static mesh", which is a
/// fact about the file rather than about the thing.
fn readable(name: &str) -> String {
    let mut words: Vec<&str> = name
        .split(['_', '-', ' '])
        .filter(|word| !word.is_empty())
        .collect();
    if words
        .first()
        .is_some_and(|first| matches!(*first, "SM" | "SK" | "SKM" | "sm" | "sk"))
    {
        words.remove(0);
    }
    if words.is_empty() {
        return name.to_owned();
    }
    words.join(" ")
}

fn count(many: u64, one: &str, more: &str) -> String {
    if many == 1 {
        format!("1 {one}")
    } else {
        format!("{many} {more}")
    }
}

/// The entry for a mesh nothing looked at: the facts, in a sentence, and
/// the fact that they are all there is.
fn measured(look: &Look) -> String {
    let size = look.size();
    format!(
        "{triangles} triangles across {meshes} and {materials}, {x:.2} x {y:.2} x {z:.2} in the \
         file's units, {textures}. No model has looked at it, so this line is the measurements \
         alone.",
        triangles = look.triangles,
        meshes = count(look.meshes, "mesh", "meshes"),
        materials = count(look.materials, "material", "materials"),
        x = size[0],
        y = size[1],
        z = size[2],
        textures = if look.images.is_empty() {
            String::from("no texture bound")
        } else {
            format!("textured from {}", look.images.join(", "))
        },
    )
}

/// **The hosted route: a chat completion with a picture in it.**
///
/// `curl` rather than an HTTP client, for the reason `tar` opens the
/// archives: this package has no dependencies, TLS is not a thing to
/// hand-roll, and curl is already on macOS, on Windows 10 build 1803 and
/// later, and on every Linux anybody runs this on.
///
/// The request goes in a file and the key goes in a config file, neither
/// of them on the command line. The body because a base64 picture is
/// half a megabyte and a command line is not; the key because a command
/// line is readable by every process on the machine.
fn ask(
    cache: &Cache,
    model: &str,
    key: &str,
    subject: &Subject<'_>,
    look: &Look,
    picture: &Path,
    digest: &str,
) -> Result<String, String> {
    let curl = on_path("curl").ok_or_else(|| NO_CURL.to_owned())?;
    let bytes = std::fs::read(picture)
        .map_err(|err| format!("cannot read the picture {}: {err}", picture.display()))?;
    let body = cache.dex_file(digest, "request.json");
    let answer = cache.dex_file(digest, "response.json");
    let config = cache.dex_file(digest, "curl.config");
    let asked = prompt(subject, look);
    // The prompt on its own, beside the request that carries it. The
    // request is the same text with a megabyte of base64 picture wrapped
    // round it, and "what was this asked?" is a question somebody reads
    // the answer to rather than greps a data URI for.
    fsx::write(&cache.dex_file(digest, "prompt.txt"), &asked)?;
    fsx::write(&body, &request(model, &asked, &bytes))?;
    fsx::write(
        &config,
        &format!(
            "url = \"{url}\"\n\
             header = \"Authorization: Bearer {key}\"\n\
             header = \"Content-Type: application/json\"\n\
             header = \"X-Title: space-trucking art dex\"\n\
             data-binary = \"@{body}\"\n\
             output = \"{answer}\"\n\
             write-out = \"%{{http_code}}\"\n\
             max-time = 180\n\
             silent\n\
             show-error\n",
            url = std::env::var("OPENROUTER_URL").unwrap_or_else(|_| ENDPOINT.to_owned()),
            body = forward_slashes(&body),
            answer = forward_slashes(&answer),
        ),
    )?;
    let output = Command::new(curl).arg("--config").arg(&config).output();
    // Whatever happened, the key does not stay on the disk. Best effort:
    // a config file that cannot be removed is not a reason to lose the
    // description, and it is in a gitignored cache either way.
    let _ = std::fs::remove_file(&config);
    let output = output.map_err(|err| format!("cannot run curl: {err}"))?;
    let status = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() {
        return Err(format!(
            "curl could not reach {model}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let said = fsx::read_to_string(&answer)?;
    if status != "200" {
        return Err(format!(
            "{model} answered {status}: {}",
            refusal(&said).unwrap_or_else(|| said.trim().chars().take(200).collect())
        ));
    }
    let description = content(&said).ok_or_else(|| {
        format!(
            "{model} answered 200 and no description{}. The answer is at {}",
            stopped(&said).unwrap_or_default(),
            answer.display()
        )
    })?;
    // The request body is the prompt with a megabyte of base64 picture
    // round it, and both halves are already on the disk beside it — the
    // prompt as text and the picture as a PNG. It is worth keeping only
    // while something is wrong, so it goes when nothing is.
    let _ = std::fs::remove_file(&body);
    Ok(description)
}

/// **The body of the request: the prompt, and the picture as a data
/// URI.**
///
/// Two numbers in it, and both were learned from an answer rather than
/// chosen. `max_tokens` was 220 — three sentences is about ninety — and
/// the first real call came back `finish_reason: length` with
/// `content: null`, because the model on the other end thinks before it
/// answers and the thinking is billed against the same budget. So the
/// budget is now large enough for a model to think and then answer, and
/// `reasoning.enabled` is false to ask the ones that can turn it off to
/// turn it off. A model that ignores that spends more tokens; a model
/// that honours it costs less than the old number did.
fn request(model: &str, prompt: &str, picture: &[u8]) -> String {
    format!(
        "{{\"model\":{model},\"max_tokens\":1200,\"temperature\":0.2,\
         \"reasoning\":{{\"enabled\":false}},\"messages\":[\
         {{\"role\":\"system\",\"content\":{system}}},\
         {{\"role\":\"user\",\"content\":[\
         {{\"type\":\"text\",\"text\":{prompt}}},\
         {{\"type\":\"image_url\",\"image_url\":{{\"url\":{picture}}}}}]}}]}}",
        model = json::quoted(model),
        system = json::quoted(SYSTEM),
        prompt = json::quoted(prompt),
        picture = json::quoted(&format!("data:image/png;base64,{}", base64(picture))),
    )
}

const SYSTEM: &str = "You write catalogue entries for a library of low-poly game art. \
     Every picture you are shown is one 3D asset rendered four times, turned a quarter of a \
     turn between views, on a plain background. Answer with the entry and nothing else: no \
     preamble, no markdown, no lists, no quotation marks.";

/// The description out of an answer, at `choices[0].message.content`.
fn content(said: &str) -> Option<String> {
    json::parse(said)
        .ok()?
        .get("choices")?
        .at(0)?
        .get("message")?
        .get("content")?
        .text()
        .map(str::to_owned)
}

/// **Why an answer carried no description**, when the answer says.
///
/// Written because of the first real call this ever made: a 200, a
/// complete and well-formed answer, and `content: null` — the model had
/// spent every token it was given thinking out loud and had none left to
/// say anything with. "No description" on its own would have sent
/// somebody looking at the picture, the prompt and the JSON writer, all
/// three of which were fine.
fn stopped(said: &str) -> Option<String> {
    let parsed = json::parse(said).ok()?;
    let choice = parsed.get("choices")?.at(0)?;
    let reason = choice.get("finish_reason").and_then(Json::text)?;
    let thought = choice
        .get("message")
        .and_then(|message| message.get("reasoning"))
        .and_then(Json::text)
        .map_or(0, str::len);
    Some(if thought > 0 {
        format!(
            ": it stopped for `{reason}` having spent {thought} characters thinking first.\n  \
             A model that thinks out loud wants a bigger budget than three sentences, or\n  \
             `--model` pointed at one that does not"
        )
    } else {
        format!(": it stopped for `{reason}`")
    })
}

/// The sentence out of a refusal, so a 401 says "No auth credentials
/// found" rather than "401".
fn refusal(said: &str) -> Option<String> {
    let parsed = json::parse(said).ok()?;
    parsed
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Json::text)
        .map(str::to_owned)
}

const NO_CURL: &str = "\
no curl: a hosted model is reached over HTTPS, and this package has no HTTP client.

  curl is already installed on macOS, on Windows 10 build 1803 and later, and on
  most Linux distributions — the same bargain `tar` gets for opening the packs. If
  this machine has not got it, either install it or set $ART_DESCRIBER to a program
  run as `<program> <prompt.txt> <picture.png>` that prints a description.";

/// A path as curl's config file wants to read it. Backslashes are escapes
/// inside a quoted value there, so a Windows path written verbatim turns
/// `C:\Users` into `C:Users` — and curl then reports a file that is not
/// there. Windows opens a path with forward slashes perfectly well.
fn forward_slashes(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Base64, the sixty-four characters and the padding. Twenty lines
/// because the alternative is a dependency for twenty lines.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut block = [0u8; 3];
        block[..chunk.len()].copy_from_slice(chunk);
        let packed = (u32::from(block[0]) << 16) | (u32::from(block[1]) << 8) | u32::from(block[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (packed >> (18 - 6 * index)) & 0x3f;
                out.push(char::from(ALPHABET[sextet as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Where a program is on `PATH`, if it is on it. A copy of the search in
/// [`crate::convert`], which keeps its own for the converter.
fn on_path(program: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_owned()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Bounds;

    fn look() -> Look {
        Look {
            triangles: 412,
            meshes: 1,
            materials: 1,
            images: vec!["PolygonSciFiSpace_Texture_01_A.png".to_owned()],
            bounds: Some(Bounds {
                mid: [0.0, 0.3, 0.0],
                half: [0.363, 0.306, 0.336],
            }),
        }
    }

    /// **The describer is told the name of the thing it is describing.**
    ///
    /// This is the guard the whole module is shaped around. A vision
    /// model shown a picture and asked what it is answers with the
    /// category — "a low-poly tree" — which is precisely what the file
    /// name already said and precisely the sentence that makes a
    /// catalogue of four hundred trees useless. The name, the pack and
    /// the measurements go in the prompt, and the instruction is to spend
    /// the sentence on what the picture adds.
    #[test]
    fn the_prompt_names_the_asset_it_is_asking_about() {
        let asked = prompt(
            &Subject {
                name: "SM_Tree_Pine_04",
                pack: "POLYGON Nature",
            },
            &look(),
        );
        assert!(asked.contains("SM_Tree_Pine_04"), "{asked}");
        assert!(
            asked.contains("Tree Pine 04"),
            "the readable form of the name is not in the prompt:\n{asked}"
        );
        assert!(asked.contains("POLYGON Nature"), "{asked}");
        assert!(asked.contains("412 triangles"), "{asked}");
        assert!(asked.contains("0.73 x 0.61 x 0.67"), "{asked}");
        assert!(
            asked.contains("PolygonSciFiSpace_Texture_01_A.png"),
            "{asked}"
        );
        // And the instructions that stop the answer being the name again.
        assert!(asked.contains("do not repeat the name"), "{asked}");
        assert!(asked.contains("This is"), "{asked}");
        assert!(asked.contains(&dex::LONGEST.to_string()), "{asked}");
    }

    /// **A mesh that rendered untextured says so in its own prompt.**
    /// Otherwise the model describes the grey of an unpainted import as
    /// though it were the pack's palette, and the catalogue records a
    /// colour the asset does not have.
    #[test]
    fn a_preview_that_bore_no_texture_does_not_ask_about_colour_as_though_it_did() {
        let asked = prompt(
            &Subject {
                name: "SM_Crate",
                pack: "A Pack",
            },
            &Look {
                images: Vec::new(),
                ..look()
            },
        );
        assert!(asked.contains("no texture reached it"), "{asked}");
    }

    /// **The request is JSON a server will accept, with the picture in
    /// it.** Built by hand, so the guard is that the hand-built thing
    /// parses and carries what it should — a prompt full of file names
    /// and quotation marks included.
    #[test]
    fn the_request_is_json_with_the_prompt_and_the_picture_in_it() {
        let body = request(
            "some/model",
            "describe \"SM_Crate\"\nplease",
            &[0xff, 0xd8, 0x00],
        );
        let parsed = json::parse(&body).unwrap_or_else(|why| panic!("{why}\n{body}"));
        assert_eq!(parsed.get("model").and_then(Json::text), Some("some/model"));
        let content = parsed
            .get("messages")
            .and_then(|messages| messages.at(1))
            .and_then(|message| message.get("content"))
            .expect("a user message");
        assert_eq!(
            content
                .at(0)
                .and_then(|part| part.get("text"))
                .and_then(Json::text),
            Some("describe \"SM_Crate\"\nplease"),
            "the prompt did not survive being made into JSON"
        );
        let url = content
            .at(1)
            .and_then(|part| part.get("image_url"))
            .and_then(|image| image.get("url"))
            .and_then(Json::text)
            .expect("a picture");
        assert_eq!(url, "data:image/png;base64,/9gA");
    }

    /// **Base64 is base64.** The picture is the whole of what the model
    /// is being paid to look at, and an encoder that is wrong in the last
    /// three bytes produces an image nothing can decode and a description
    /// of nothing.
    #[test]
    fn a_picture_is_encoded_the_way_every_decoder_expects() {
        for (bytes, wanted) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
            (&[0x00, 0xff, 0x80], "AP+A"),
        ] {
            assert_eq!(base64(bytes), wanted, "{bytes:?}");
        }
    }

    /// **An answer is read out of the shape it comes in, and a refusal is
    /// read too.** A key that is not paying for that model answers 402
    /// with a sentence saying so, and "402" on its own sends somebody to
    /// the wrong page.
    #[test]
    fn an_answer_and_a_refusal_are_both_read() {
        let answer =
            r#"{"choices":[{"message":{"content":"A squat crate with a hazard stripe."}}]}"#;
        assert_eq!(
            content(answer).as_deref(),
            Some("A squat crate with a hazard stripe.")
        );
        assert_eq!(content("not json at all"), None);
        assert_eq!(
            refusal(r#"{"error":{"message":"Insufficient credits","code":402}}"#).as_deref(),
            Some("Insufficient credits")
        );
    }

    /// **A model that spent its whole budget thinking says so.**
    ///
    /// The first real call this pipeline ever made came back exactly like
    /// this: HTTP 200, a well-formed answer, `finish_reason: length`, a
    /// thousand characters of reasoning and `content: null`. Everything a
    /// person would go and check — the picture, the prompt, the JSON
    /// writer — was fine, and the one fact that explains it is in the
    /// answer. So it is read out and put in the complaint.
    #[test]
    fn a_model_that_spent_its_budget_thinking_is_told_apart_from_one_that_said_nothing() {
        let thinking = r#"{"choices":[{"index":0,"finish_reason":"length","message":
            {"role":"assistant","content":null,"reasoning":"Let me look at the image. It is a
             cube-shaped sci-fi crate with a yellow lid."}}]}"#;
        assert_eq!(content(thinking), None, "there is no description in it");
        let why = stopped(thinking).expect("the answer says why");
        assert!(why.contains("length"), "{why}");
        assert!(why.contains("thinking first"), "{why}");
        assert!(why.contains("--model"), "{why}");

        // And an answer that simply stopped says the plain thing.
        let empty = r#"{"choices":[{"finish_reason":"stop","message":{"content":""}}]}"#;
        assert_eq!(content(empty).as_deref(), Some(""));
        assert_eq!(stopped(empty).as_deref(), Some(": it stopped for `stop`"));
    }

    /// **A machine with no key still gets a catalogue, and the catalogue
    /// says what it is.** The measured line is a real answer to half the
    /// questions a person asks of a pack — how big, how heavy — and the
    /// one thing it must never do is read like a sentence somebody's
    /// model wrote.
    #[test]
    fn with_nothing_to_look_with_the_entry_is_the_measurements_and_says_so() {
        let said = measured(&look());
        assert!(said.contains("412 triangles"), "{said}");
        assert!(said.contains("0.73 x 0.61 x 0.67"), "{said}");
        assert!(said.contains("No model has looked at it"), "{said}");
        assert_eq!(Describer::Measurements.describe(), dex::UNDESCRIBED);
    }

    /// **A file name is read the way somebody would say it.** The `SM_`
    /// an exporter writes is a fact about the file, not about the thing,
    /// and a model told the asset is called "SM" spends a clause on it.
    #[test]
    fn a_file_name_is_offered_to_the_model_in_english_as_well() {
        assert_eq!(readable("SM_Prop_Crate_01"), "Prop Crate 01");
        assert_eq!(readable("SK_Character_Rig"), "Character Rig");
        assert_eq!(readable("Wall-Panel 02"), "Wall Panel 02");
        assert_eq!(readable("SM"), "SM", "a name that is only a prefix stays");
    }

    /// **A Windows path goes into a curl config file as curl reads
    /// them.** Backslashes are escapes inside a quoted value there, so
    /// the verbatim path would arrive as `C:Usersyouart` — and curl would
    /// report a request body that is not there, on the owner's machine
    /// and on no test machine.
    #[test]
    fn a_path_in_the_curl_config_is_one_curl_can_open() {
        let written = forward_slashes(Path::new(r"C:\Users\you\art\cache\dex\abc\request.json"));
        assert_eq!(written, "C:/Users/you/art/cache/dex/abc/request.json");
        assert!(!written.contains('\\'), "{written}");
    }
}
