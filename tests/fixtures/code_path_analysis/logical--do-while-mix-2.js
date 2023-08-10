/*expected
initial->s1_1->s1_2->s1_3->s1_4->s1_2->s1_2;
s1_3->s1_5;
s1_4->s1_5->final;
*/
do {
    foo();
} while (a || b && c);

/*DOT
digraph {
    node[shape=box,style="rounded,filled",fillcolor=white];
    initial[label="",shape=circle,style=filled,fillcolor=black,width=0.25,height=0.25];
    final[label="",shape=doublecircle,style=filled,fillcolor=black,width=0.25,height=0.25];
    s1_1[label="Program\nDoWhileStatement"];
    s1_2[label="BlockStatement\nExpressionStatement\nCallExpression\nIdentifier (foo)\nLogicalExpression\nIdentifier (a)"];
    s1_3[label="LogicalExpression\nIdentifier (b)"];
    s1_4[label="Identifier (c)"];
    s1_5[label="DoWhileStatement:exit\nProgram:exit"];
    initial->s1_1->s1_2->s1_3->s1_4->s1_2->s1_2;
    s1_3->s1_5;
    s1_4->s1_5->final;
}
*/
