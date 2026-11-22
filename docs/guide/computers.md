# Your computers

[← Hangar](../../README.md) · [Guide](README.md)

With HangarAI open on two computers of the same license, one drives the other. On your
laptop, **Your computers** (« Tes ordinateurs ») in the navigator lists your PC; you open
its workspaces as if they were on the laptop, with their tiles, terminals, conversations,
board and deliveries, and the work runs on the PC. Its agents never notice you changed
seat. It comes with the paid licenses (subscription, lifetime, and the trial).

Nothing to install and nothing to open: both computers connect out to HangarAI's relay,
which puts them through to each other, and everything they say is encrypted end to end
with keys only they have.

## Connect a computer, once

1. **Open HangarAI on both**, with the same license. Each one lists the other under
   **Your computers**, by its name and its system, as soon as both are open. It is on by
   default; nothing to turn on.
2. **On the laptop, click Connect** beside the PC. A sheet shows six digits, big: « Type
   this code on PC-Salon or in Hangar Mobile ».
3. **On the PC**, a request opens by itself, with a notification: « MacBook Air wants to
   drive this PC ». Type the six digits and click **Allow**. Away from the PC? The same
   request is first in **For you** in [Hangar Mobile](mobile.md), on your paired phone:
   type the code there.
4. **Back on the laptop**: « PC-Salon accepted. Did you type the code on PC-Salon? »
   Click **Yes, open**. The PC's workspaces are listed under it, and open like any other.

That is once per computer. From then on the laptop connects by itself whenever both are
open.

> [!NOTE]
> The code is typed on the PC, not compared. A request to drive a computer is a request
> for a shell on it: whoever raised it would need you to type six digits that are on
> **their** screen. A request you did not start is the sign that someone holds your
> license or your account: refuse it, with **Refuse and don't ask again** if it comes
> back, and check your license's computers.

## Working on the PC from the laptop

Everything you do with a workspace on the laptop, you do with the PC's: type in its
terminals, follow and answer its conversations, its board and memory, merge or send back
its deliveries, make a workspace there, rename or delete one. The PC stays their owner:
its agents, its orchestration and its deliveries run there.

**On the PC**, for as long as it is driven, the status bar says **Driven from MacBook Air**
with **Disconnect**. Its own window stays yours: typing in the same terminal from both is
two keyboards on one shell. A terminal has one size, the size of the screen where it was
last typed in; the other screen draws it at that size, centred or cut, with **Take over**
to take it back.

**The PC stays awake** while a computer drives it (the system, not the screen), and is let
go when the last one leaves. **Settings › Computers › Keep awake while driven** turns it
off.

## When the link drops

The laptop changed network, the train went through a tunnel, the relay restarted: the
laptop connects again by itself, at once and then every few seconds, and as soon as the
PC is listed again. Each terminal on screen catches up with what it printed meanwhile,
and the workspaces are read again. The agents on the PC never noticed.

What the laptop shows of the PC:

| You see | It means |
|---|---|
| its workspaces, live | connected |
| **Connecting…** | the link is being made |
| **PC-Salon does not answer**, **Try again** | it is listed but did not answer; the reason is in the tooltip |
| **PC-Salon is offline** · Off, asleep, or HangarAI closed · seen 3 h ago | it left the list; « seen » is when this laptop last saw it |
| **No connection to the relay** | this laptop cannot reach the relay: the PC may well be there |
| **PC-Salon went to sleep**, **HangarAI closed on PC-Salon** | the PC said goodbye; it is driven again once it is back |
| **PC-Salon ended the session**, **Reconnect** | someone clicked Disconnect on the PC: nothing comes back by itself |
| **PC-Salon no longer recognises this computer** | the laptop was removed on the PC; asking again takes a new code |

A sleeping PC cannot be woken through the relay: nothing of it is awake to hear.

## Settings › Computers

On each computer:

- **Name**: what your other computers list it as; the system's name until you give one.
- **Show to my other computers**: on by default. Off, the others do not see this computer
  at all, as if it were switched off, and it does not see them.
- **Let them ask to drive this one**: off, this computer only drives the others.
- **Keep awake while driven**.
- **Allowed to drive this one**: each computer you allowed, with its last use and
  **Remove**. Removed, its key opens nothing any more and its sessions end at once.
- **Driven from this one**: the computers this one drives, online or « seen … », with
  **Forget**.

## Privacy and security

- Both computers connect **out** to the relay on HangarAI's account server; nothing
  listens on either. On the laptop, the PC is reached through a port of its own on
  `127.0.0.1`, and every request on it needs the session's ticket: another program on the
  laptop, or a web page, gets nothing.
- The relay sees each computer's name and system, so that the others can list it, and when
  it is connected; it tells them to the other computers of your license and nobody else.
  Of what two computers say to each other (workspaces, terminals, conversations, files) it
  sees only the size and timing. It keeps none of it. See the
  [privacy page](https://hangarai.dev/privacy.html#your-computers).
- A computer you allowed can do everything you can on the PC: that is why it is allowed
  there, once, with a code; why the PC says who drives it for as long as it is driven; and
  why **Remove** takes effect at once.

The design and its reasons are in [docs/native/computers.md](../native/computers.md).
