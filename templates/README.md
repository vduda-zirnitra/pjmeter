# PJmeter

## What is this ?

This folder has been created by using [packaged-jmeter](https://gitlab.bordeaux.sqli.com/vduda/packaged-jmeter/), a command line encapsulation with a Dockerised JMeter.\
Using this, you won't have to worry about specific configuration, executions, JMeter version download, project organisation and so on.

JMeter is an open-source tool designed for load testing and performance measurement of web applications and other services. It enables users to simulate multiple users, analyze system behavior under stress, and generate detailed performance reports.

The metrics you'll get from it will allow you to see how your application's performances evolve and can serve as proof of performance to the Product's Owner.

Check the link above to download a .deb to install `pjmeter` on your computer and try it out.

## How to use it ?

To use this, you first need to install `pjmeter` (check the link in [What is this ?](#what-is-this-)).

### Add the project to your local configuration

You need to explicitely tell pjmeter what projects you're working on. To do it, you have to add your projects with :
```shell
pjmeter project add --label my-project --path /absolute/path/to/my/project
```

:information_source: check out documentation to update / remove a project in pjmeter configuration.

### Design / View a test plan

To see what a test plan actually do, you can run JMeter in windowed mode using :
```shell
pjmeter design
```

JMeter will open, and you'll be able to see, edit or create test plans in a human friendly way.

:warning: As per JMeter documentation, windowed mode should not be used for actual load testing, only for design / debug.

### Run test plans

To make JMeter execute all your test plans, simply use :
```shell
pjmeter run --project my-project --environment local
```

Every test plan in `plans/` directory will be executed.

You could also specify one or several specific plans to execute :
```shell
pjmeter run --project my-project --environment local --plans my-plan-1.jmx,my-plan-2.jmx
```
:information_source: Note that your plans have to be coma separated.

Once JMeter finishes executing your plan, you'll find the resulting reports in `results/` directory. The result directory will be named as : plan-file-name-environment-YYYYMMDDTHHmmSS_report.

For example, for my-plan-1.jmx executed just above : `my-plan-1_local_20250722T190019_result`

## More about Packaged-Jmeter

pjmeter is meant to help you easily set up JMeter test plans on any project. Check out `jmeter help` to see all the commands you can run with it or check the project's Gitlab (link in [What is this ?](#what-is-this-)). 