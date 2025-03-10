use crate::recording::{ExtraData, Frame, Move, PhysicsData, Recording, SpeedrunHeader};
use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::{escaped, is_a, tag, take_while};
use nom::character::complete::{char, one_of};
use nom::combinator::{cut, map, opt};
use nom::error::{context, convert_error, ParseError, VerboseError};
use nom::multi::{many0, many1, separated_list};
use nom::number::complete::{double, float};
use nom::sequence::{delimited, preceded, separated_pair, terminated, tuple};
use nom::{Err, IResult};
use regex::Regex;
use std::fmt::Debug;
use std::io::Write;
use std::str::FromStr;
use crate::error::Result;
use crate::error::ErrorKind::{GenericError, GenericError2};

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: String,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TasFile {
    pub mission: String,
    pub sequences: Vec<Sequence>,
}

impl TasFile {
    pub fn from_rec(recording: Recording) -> TasFile {
        TasFile {
            mission: recording.mission,
            sequences: vec![Sequence {
                name: "Imported".into(),
                frames: recording.frames,
            }],
        }
    }

    pub fn into_rec(self) -> Recording {
        let mut frames = vec![];
        let mut seqs = self.sequences;
        for sequence in &mut seqs {
            frames.append(&mut sequence.frames);
        }

        Recording {
            mission: self.mission,
            frames,
        }
    }

    pub fn parse(input: String) -> Result<TasFile> {
        let re = Regex::new(r"//.*").map_err(|e| GenericError2(format!("Regex Error: {}", e)))?;
        let formatted = re.replace_all(input.as_str(), "");
        match tasfile::<VerboseError<&str>>(&formatted) {
            Ok((_, tf)) => Ok(tf),
            Err(Err::Error(e)) | Err(Err::Failure(e)) => {
                Err(GenericError2(convert_error(&formatted, e)).into())
            }
            _ => Err(GenericError("Unknown Error").into()),
        }
    }

    fn print_move<T>(&self, opt_mv: &Option<Move>, out: &mut T) -> Result<()>
    where
        T: Write,
    {
        if let Some(mv) = opt_mv {
            out.write_fmt(format_args!("      {{\n"))?;
            out.write_fmt(format_args!(
                "         camera ({} {} {})\n",
                mv.yaw.map(|val| format_args!("{}", val).to_string()).unwrap_or("null".to_string()),
                mv.pitch.map(|val| format_args!("{}", val).to_string()).unwrap_or("null".to_string()),
                mv.roll.map(|val| format_args!("{}", val).to_string()).unwrap_or("null".to_string())
            ))?;
            out.write_fmt(format_args!(
                "         move ({} {} {})\n",
                mv.mx, mv.my, mv.mz
            ))?;
            if !mv.freelook {
                out.write_fmt(format_args!(
                    "         freelook 0\n"
                ));
            }
            out.write_fmt(format_args!(
                "         triggers ({} {} {} {} {} {})\n",
                mv.triggers[0] as u8,
                mv.triggers[1] as u8,
                mv.triggers[2] as u8,
                mv.triggers[3] as u8,
                mv.triggers[4] as u8,
                mv.triggers[5] as u8
            ))?;
            out.write_fmt(format_args!("      }}\n"))?;
        } else {
            out.write_fmt(format_args!("      {{}}\n"))?;
        }
        Ok(())
    }

    fn print_extra<T>(
        &self,
        extra: &ExtraData,
        out: &mut T,
    ) -> Result<()>
    where
        T: Write
    {
        if let Some(speedrun_header) = &extra.speedrun_header {
            out.write_fmt(format_args!(
                "         header ({} {} {} {} {} {} {} {})\n",
                speedrun_header.version_major,
                speedrun_header.version_minor,
                speedrun_header.reference_system_time,
                speedrun_header.epoch_offset,
                speedrun_header.resolution_horz,
                speedrun_header.resolution_vert,
                speedrun_header.fov,
                speedrun_header.fps,
            ))?;
        }
        if let Some(sys_time_delta) = &extra.sys_time_delta {
            out.write_fmt(format_args!("         sysDelta {}\n", sys_time_delta))?;
        }
        if let Some(game_elapsed) = &extra.game_elapsed {
            out.write_fmt(format_args!("         gameElapsed {}\n", game_elapsed))?;
        }
        if let Some(physics_data) = &extra.physics_data {
            out.write_fmt(format_args!(
                "         physics ({} {} {} {} {} {} {} {} {})\n",
                physics_data.position_x,
                physics_data.position_y,
                physics_data.position_z,
                physics_data.velocity_x,
                physics_data.velocity_y,
                physics_data.velocity_z,
                physics_data.angular_velocity_x,
                physics_data.angular_velocity_y,
                physics_data.angular_velocity_z,
            ))?;
        }
        Ok(())
    }

    fn print_sequence<T>(
        &self,
        seq: &Sequence,
        out: &mut T,
        elapsed: &mut u32,
    ) -> Result<()>
    where
        T: Write,
    {
        out.write_fmt(format_args!(
            "   {{\n      {}\n",
            TasFile::escape(&seq.name)
        ))?;

        let mut i = 0;
        while i < seq.frames.len() {
            let frame = &seq.frames[i];

            *elapsed += u32::from(frame.delta);
            if frame.has_data() {
                out.write_fmt(format_args!(
                    "      moveframe {} ms //{}\n",
                    frame.delta, elapsed
                ))?;
                self.print_move(&frame.moves[0], out)?;
                self.print_move(&frame.moves[1], out)?;
                if let Some(extra) = &frame.extra_data {
                    out.write_fmt(format_args!("      {{\n"))?;
                    self.print_extra(extra, out)?;
                    out.write_fmt(format_args!("      }}\n"))?;
                }
            } else {
                // See how many in a row we have
                let mut combined = 0;
                for j in i..seq.frames.len() {
                    if seq.frames[j].has_data() {
                        break;
                    }
                    if seq.frames[j].delta != frame.delta {
                        break;
                    }
                    combined += 1
                }
                if combined > 1 {
                    i += combined - 1;
                    out.write_fmt(format_args!(
                        "      frames {} {} ms // {} -> {}\n",
                        combined,
                        frame.delta,
                        elapsed,
                        *elapsed + u32::from(frame.delta) * (combined - 1) as u32
                    ))?;
                    *elapsed += u32::from(frame.delta) * (combined - 1) as u32;
                } else {
                    out.write_fmt(format_args!(
                        "      frame {} ms // {}\n",
                        frame.delta, elapsed
                    ))?;
                }
            }

            i += 1;
        }

        out.write_fmt(format_args!("   }}\n"))?;
        Ok(())
    }

    pub fn print<T>(&self, out: &mut T) -> Result<()>
    where
        T: Write,
    {
        out.write_fmt(format_args!("{{\n   {}\n", TasFile::escape(&self.mission)))?;

        let mut elapsed: u32 = 0;

        for seq in &self.sequences {
            self.print_sequence(seq, out, &mut elapsed)?;
        }
        out.write_fmt(format_args!("}}\n"))?;
        Ok(())
    }

    pub fn escape(value: &str) -> String {
        format!("\"{}\"", value.replace("\\", "\\\\").replace("\"", "\\\""))
    }
}

fn delim_context_cut<'a, E: ParseError<&'a str>, F, G, H, O1, O2, O3>(
    name: &'static str,
    before: F,
    inner: G,
    after: H,
) -> impl Fn(&'a str) -> IResult<&'a str, O2, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O1, E>,
    G: Fn(&'a str) -> IResult<&'a str, O2, E>,
    H: Fn(&'a str) -> IResult<&'a str, O3, E>,
{
    context(name, preceded(before, cut(terminated(inner, after))))
}

fn ws_before<'a, E: ParseError<&'a str>, F, O>(
    inner: F,
) -> impl Fn(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    preceded(opt(sp), inner)
}

fn ws_wrap<'a, E: ParseError<&'a str>, F, O>(inner: F) -> impl Fn(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    preceded(opt(sp), terminated(inner, opt(sp)))
}

fn sp<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    let chars = " \t\r\n";

    // nom combinators like `take_while` return a function. That function is the
    // parser,to which we can pass the input
    take_while(move |c| chars.contains(c))(i)
}

fn parse_str<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    escaped(is_not("\"\\"), '\\', one_of("\"n\\"))(i)
}

fn string<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    context(
        "string",
        preceded(char('\"'), cut(terminated(parse_str, char('\"')))),
    )(i)
}

fn empty_frame<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Vec<Frame>, E> {
    ws_before(map(terminated(double, preceded(sp, tag("ms"))), |ms| {
        vec![Frame {
            moves: [None, None],
            delta: ms as u16,
            extra_data: None,
        }]
    }))(i)
}

fn empty_frames<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Vec<Frame>, E> {
    ws_before(map(
        tuple((double, opt(delimited(sp, double, preceded(sp, tag("ms")))))),
        |(n, ms)| {
            let mut v: Vec<Frame> = Vec::with_capacity(n as usize);
            for _ in 0..n as usize {
                v.push(Frame {
                    moves: [None, None],
                    delta: ms.map(|f| f as u16).unwrap_or(1),
                    extra_data: None,
                })
            }
            v
        },
    ))(i)
}

fn float3<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, (f64, f64, f64), E> {
    delim_context_cut(
        "float3",
        char('('),
        ws_wrap(tuple((
            preceded(opt(sp), double),
            preceded(opt(sp), double),
            preceded(opt(sp), double),
        ))),
        char(')'),
    )(i)
}

fn bool6<'a, E: ParseError<&'a str>>(
    i: &'a str,
) -> IResult<&'a str, (bool, bool, bool, bool, bool, bool), E> {
    delim_context_cut(
        "bool6",
        char('('),
        ws_wrap(map(
            tuple((
                preceded(opt(sp), is_a("01")),
                preceded(opt(sp), is_a("01")),
                preceded(opt(sp), is_a("01")),
                preceded(opt(sp), is_a("01")),
                preceded(opt(sp), is_a("01")),
                preceded(opt(sp), is_a("01")),
            )),
            |(a, b, c, d, e, f)| {
                (
                    a.chars().nth(0).map_or(false, |ch| ch == '1'),
                    b.chars().nth(0).map_or(false, |ch| ch == '1'),
                    c.chars().nth(0).map_or(false, |ch| ch == '1'),
                    d.chars().nth(0).map_or(false, |ch| ch == '1'),
                    e.chars().nth(0).map_or(false, |ch| ch == '1'),
                    f.chars().nth(0).map_or(false, |ch| ch == '1'),
                )
            },
        )),
        char(')'),
    )(i)
}

fn opt_float<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Option<f64>, E> {
    alt((
        map(double, |d| Some(d)),
        map(tag("null"), |_| None),
    ))(i)
}

fn move_inner<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Move, E> {
    ws_wrap(map(
        tuple((
            preceded(tag("camera"), ws_wrap(
                delim_context_cut(
                    "camera",
                    char('('),
                    tuple((
                        preceded(opt(sp), opt_float),
                        preceded(opt(sp), opt_float),
                        preceded(opt(sp), opt_float),
                    )),
                    char(')'),
                )
            )),
            preceded(tag("move"), ws_wrap(float3)),
            opt(preceded(tag("freelook"), ws_wrap(decimal::<u8, E>))),
            preceded(tag("triggers"), ws_wrap(bool6)),
        )),
        |((yaw, pitch, roll), (mx, my, mz), freelook, triggers)| Move {
            yaw,
            pitch,
            roll,
            mx,
            my,
            mz,
            freelook: freelook.unwrap_or(1) != 0,
            triggers: [
                triggers.0, triggers.1, triggers.2, triggers.3, triggers.4, triggers.5,
            ],
        },
    ))(i)
}

fn move_<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Option<Move>, E> {
    delim_context_cut("move", char('{'), ws_before(opt(move_inner)), char('}'))(i)
}

fn decimal<'a, T: FromStr, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, T, E>
where <T as FromStr>::Err: Debug {
    map(
        many1(one_of("0123456789")),
        |chars| {
            T::from_str(
                &chars.into_iter().collect::<String>()
            ).expect("number parse should work")
        }
    )(i)
}

fn speedrun_header<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, SpeedrunHeader, E> {
    delim_context_cut("speedrun_header", char('('), map(tuple((
        ws_before(decimal::<u8, E>),
        ws_before(decimal::<u8, E>),
        ws_before(decimal::<u64, E>),
        ws_before(decimal::<u64, E>),
        ws_before(decimal::<u16, E>),
        ws_before(decimal::<u16, E>),
        ws_before(float),
        ws_before(float),
    )), |(version_major, version_minor, reference_system_time, epoch_offset, resolution_horz, resolution_vert, fov, fps)| {
        SpeedrunHeader {
            version_major,
            version_minor,
            reference_system_time,
            epoch_offset,
            resolution_horz,
            resolution_vert,
            fov,
            fps,
        }
    }), char(')'))(i)
}

fn physics_data<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, PhysicsData, E> {
    delim_context_cut("physics_data", char('('), map(tuple((
        ws_before(double),
        ws_before(double),
        ws_before(double),
        ws_before(double),
        ws_before(double),
        ws_before(double),
        ws_before(double),
        ws_before(double),
        ws_before(double),
    )), |(position_x, position_y, position_z, velocity_x, velocity_y, velocity_z, angular_velocity_x, angular_velocity_y, angular_velocity_z)| {
        PhysicsData {
            position_x,
            position_y,
            position_z,
            velocity_x,
            velocity_y,
            velocity_z,
            angular_velocity_x,
            angular_velocity_y,
            angular_velocity_z,
        }
    }), char(')'))(i)
}

fn extra_data<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, ExtraData, E> {
    delim_context_cut("extra_data", char('{'), ws_wrap(
        map(
            tuple((
                opt(ws_before(preceded(tag("header"), ws_before(speedrun_header)))),
                opt(ws_before(preceded(tag("sysDelta"), ws_before(decimal::<u32, E>)))),
                opt(ws_before(preceded(tag("gameElapsed"), ws_before(decimal::<u32, E>)))),
                opt(ws_before(preceded(tag("physics"), ws_before(physics_data))))
            )),
            |(speedrun_header, sys_time_delta, game_elapsed, physics_data)| {
                ExtraData {
                    speedrun_header,
                    sys_time_delta,
                    game_elapsed,
                    physics_data
                }
            }
        )
    ), char('}'))(i)
}

fn move_frame<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Vec<Frame>, E> {
    map(
        tuple((
            ws_before(terminated(double, preceded(sp, tag("ms")))),
            ws_before(move_),
            ws_before(move_),
            opt(ws_before(extra_data)),
        )),
        |(ms, mv0, mv1, extra_data)| {
            vec![Frame {
                moves: [mv0, mv1],
                delta: ms as u16,
                extra_data,
            }]
        },
    )(i)
}

fn frame<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Vec<Frame>, E> {
    alt((
        preceded(tag("frames"), context("frames", cut(empty_frames))),
        preceded(tag("frame"), context("frame", cut(empty_frame))),
        preceded(tag("moveframe"), context("moveframe", cut(move_frame))),
    ))(i)
}

fn sequence<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Sequence, E> {
    delim_context_cut(
        "sequence",
        char('{'),
        ws_wrap(map(
            separated_pair(string, sp, separated_list(sp, frame)),
            |(name, mut frames)| {
                let mut collected = vec![];
                for list in &mut frames {
                    collected.append(list);
                }
                Sequence {
                    name: name.into(),
                    frames: collected,
                }
            },
        )),
        char('}'),
    )(i)
}

fn tasfile<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, TasFile, E> {
    delim_context_cut(
        "tasfile",
        char('{'),
        map(
            tuple((ws_wrap(string), many0(ws_wrap(sequence)))),
            |(mission, sequences)| TasFile {
                mission: mission.into(),
                sequences,
            },
        ),
        char('}'),
    )(i)
}
