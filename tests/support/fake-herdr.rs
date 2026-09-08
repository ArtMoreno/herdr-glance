use std::{env,fs::{self,OpenOptions},io::Write,thread,time::Duration};
fn main(){
    let args:Vec<String>=env::args().skip(1).collect();
    let dir=env::current_exe().unwrap().parent().unwrap().to_owned();
    let mut log=OpenOptions::new().create(true).append(true).open(dir.join("calls.log")).unwrap();
    writeln!(log,"{}",args.join("|")).unwrap();
    assert_eq!(&args[..2],["--session","fixture"]);
    assert_eq!(args[2], if args[3]=="read" {"pane"} else {"agent"});
    match args[3].as_str(){
        "list"=>{
            if dir.join("slow").exists(){thread::sleep(Duration::from_secs(6));}
            if dir.join("broken").exists(){println!("not JSON");return;}
            let fixture=include_str!("../fixtures/agents.json");
            let data=if dir.join("replaced").exists(){fixture.replace("term-claude","new-terminal")}else{fixture.to_owned()};
            let data=if dir.join("duplicate").exists(){data.replace("outside workspace","design / review")}else{data};
            println!("{data}");
        }
        "read"=>{
            assert_eq!(args[5],"--source");assert_eq!(args[7],"--lines");
            assert!(["recent-unwrapped","detection"].contains(&args[6].as_str()));
            assert!(["40","200"].contains(&args[8].as_str()));
            assert!(["w-demo:p1","w-demo:p2","w-demo:p3","w-demo:p4","w-demo:p5"].contains(&args[4].as_str()));
            let text=if args[6]=="detection"{"Which icon set?"}else if args[4]!="w-demo:p2"{"Fixture output.\\nReady for review."}else if args[8]=="40"{""}else{"first\\nsecond"};
            print!("{}", text.replace("\\n", "\n"));
        }
        "focus"=>{assert!(["design / review","w-demo:p2"].contains(&args[4].as_str()));println!("{{\"result\":{{\"type\":\"ok\"}}}}");}
        _=>{fs::write(dir.join("unexpected-mutation"),args.join(" ")).unwrap();panic!("unexpected command");}
    }
}
