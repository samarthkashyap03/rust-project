// The quiz module from Rust 2 (`mod quiz` in quiz/src/main.rs, branch
// rust2) as a file of its own. Use it with `mod quiz;`: make an mpsc
// channel, send `quiz::Answer`s from any thread and give the receiver to
// `quiz::play`. Needs serde (feature "derive") and serde_json.

use std::fmt;
use std::fs;
use std::io;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

// The four face buttons of the phone controller.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Answer {
    A,
    B,
    X,
    Y,
}

#[derive(Deserialize)]
pub struct Question {
    pub text: String,
    pub options: [String; 4],
    pub correct: Answer,
}

impl Question {
    fn is_correct(&self, a: Answer) -> bool {
        // What if Answer did not derive PartialEq?
        self.correct == a
    }
}

impl fmt::Display for Question {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.text)?;
        writeln!(f, "  A) {}", self.options[0])?;
        writeln!(f, "  B) {}", self.options[1])?;
        writeln!(f, "  X) {}", self.options[2])?;
        write!(f, "  Y) {}", self.options[3])
    }
}

#[derive(Debug, PartialEq)]
enum State {
    Asking,
    Revealed { correct: bool },
    Finished,
}

pub struct Quiz {
    pub questions: Vec<Question>,
    current: usize,
    pub score: u32,
    results: Vec<bool>,
    state: State,
}

impl Quiz {
    pub fn new(questions: Vec<Question>) -> Quiz {
        Quiz {
            questions,
            current: 0,
            score: 0,
            results: Vec::new(),
            state: State::Asking,
        }
    }

    pub fn current(&self) -> Option<&Question> {
        match self.state {
            State::Finished => None,
            _ => self.questions.get(self.current),
        }
    }

    pub fn answer(&mut self, a: Answer) -> bool {
        let correct = self.questions[self.current].is_correct(a);
        if correct {
            self.score += 1;
        }
        self.results.push(correct);
        self.state = State::Revealed { correct };
        correct
    }

    fn time_up(&mut self) {
        self.results.push(false);
        self.state = State::Revealed { correct: false };
    }

    pub fn next(&mut self) {
        self.current += 1;
        if self.current < self.questions.len() {
            self.state = State::Asking;
        } else {
            self.state = State::Finished;
        }
    }

    pub fn wrong_answers(&self) -> Vec<&Question> {
        self.questions
            .iter()
            .zip(&self.results)
            .filter(|(_, correct)| !**correct)
            .map(|(q, _)| q)
            .collect()
    }
}

pub fn parse_answer(s: &str) -> Result<Answer, String> {
    match s.trim() {
        "a" | "A" => Ok(Answer::A),
        "b" | "B" => Ok(Answer::B),
        "x" | "X" => Ok(Answer::X),
        "y" | "Y" => Ok(Answer::Y),
        other => Err(format!("'{other}' is not one of A, B, X, Y")),
    }
}

pub fn label(a: Answer) -> char {
    match a {
        Answer::A => 'A',
        Answer::B => 'B',
        Answer::X => 'X',
        Answer::Y => 'Y',
    }
}

#[cfg(test)]
fn question(text: &str, options: [&str; 4], correct: Answer) -> Question {
    Question {
        text: String::from(text),
        // Turn each &str into an owned String.
        options: options.map(String::from),
        correct,
    }
}

#[derive(Debug)]
pub enum QuizError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl From<io::Error> for QuizError {
    fn from(e: io::Error) -> QuizError {
        QuizError::Io(e)
    }
}

impl From<serde_json::Error> for QuizError {
    fn from(e: serde_json::Error) -> QuizError {
        QuizError::Json(e)
    }
}

impl fmt::Display for QuizError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QuizError::Io(e) => write!(f, "cannot read questions: {e}"),
            QuizError::Json(e) => write!(f, "invalid questions: {e}"),
        }
    }
}

pub fn load_questions(path: &str) -> Result<Vec<Question>, QuizError> {
    let text = fs::read_to_string(path)?;
    let questions = serde_json::from_str(&text)?;
    Ok(questions)
}

const SECONDS_PER_QUESTION: f64 = 10.0;

// The game loop: one round every few milliseconds until the quiz
// is over or the answers channel is closed.
pub fn play(quiz: &mut Quiz, answers: Receiver<Answer>) {
    while let Some(q) = quiz.current() {
        println!("{q}");
        let correct = q.correct;
        // Throw away answers that arrived before the question.
        while answers.try_recv().is_ok() {}

        let mut time_left = SECONDS_PER_QUESTION;
        let mut shown = 0; // the whole seconds printed last
        let mut last = Instant::now();
        loop {
            // dt: the time since the last round, in seconds
            let now = Instant::now();
            let dt = (now - last).as_secs_f64();
            last = now;
            time_left -= dt;

            match answers.try_recv() {
                Ok(a) => {
                    if quiz.answer(a) {
                        println!("Correct!\n");
                    } else {
                        println!("Wrong, it was {}.\n", label(correct));
                    }
                    break;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => return,
            }

            if time_left <= 0.0 {
                println!("Time is up, it was {}.\n", label(correct));
                quiz.time_up();
                break;
            }
            let seconds = time_left.ceil() as u32;
            if seconds != shown {
                println!("{seconds}");
                shown = seconds;
            }
            // What happens if we do not sleep?
            thread::sleep(Duration::from_millis(16));
        }
        quiz.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Quiz {
        let q = question("What is 1 + 1?", ["1", "2", "3", "4"], Answer::B);
        Quiz::new(vec![q])
    }

    #[test]
    fn correct_answer_scores_a_point() {
        let mut quiz = sample();
        assert!(quiz.answer(Answer::B));
        assert_eq!(quiz.score, 1);
        assert_eq!(quiz.state, State::Revealed { correct: true });
        quiz.next();
        assert_eq!(quiz.state, State::Finished);
        assert!(quiz.current().is_none());
    }

    #[test]
    fn unknown_input_is_an_error() {
        assert!(parse_answer("z").is_err());
        assert_eq!(parse_answer(" b "), Ok(Answer::B));
    }
}
