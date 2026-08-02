Meridian is an app to keep track of a user's finances. It can keep track of
portfolios and general spending through "trades". A trade is just an exchange of
X A -> Y B at a time t. For example, one can by 3 apples for 6 dollars, say.
Note that a trade goes one way.
This is the atomic unit underlying the entire app. Do note that a trade can be
one sided: "we trade *in* 4000 dollars", that's a trade of 0 things to 4000
dollars, like the user getting paid (we're adding money into the system).
Similarly the user making a payment is just a trade out of the system. They're
still just trades. It's important to note that "money" here is not special. It's
just a unit. Could be USD, could be euros, etc. We keep track of user portfolios
this way, but we can also keep track of a user's spendings. The app will ideally
hook into the user's bank to be able to populate this "ledger".
