//! Introspection statement parsers: SHOW (RELATIONS, LAYERS, FEATURES, TOKENS, MODELS), STATS.

use crate::ast::*;
use crate::lexer::{Keyword, Token};
use super::{Parser, ParseError};

impl Parser {
    pub(crate) fn parse_show(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword(Keyword::Show)?;

        match self.peek() {
            Token::Keyword(Keyword::Relations) => {
                self.advance();
                let mut layer = None;
                let mut with_examples = false;
                let mut mode = DescribeMode::default();

                loop {
                    match self.peek() {
                        Token::Keyword(Keyword::At) => {
                            self.advance();
                            self.expect_keyword(Keyword::Layer)?;
                            layer = Some(self.expect_u32()?);
                        }
                        Token::Keyword(Keyword::With) => {
                            self.advance();
                            self.expect_keyword(Keyword::Examples)?;
                            with_examples = true;
                        }
                        Token::Keyword(Keyword::Verbose) => {
                            self.advance();
                            mode = DescribeMode::Verbose;
                        }
                        Token::Keyword(Keyword::Brief) => {
                            self.advance();
                            mode = DescribeMode::Brief;
                        }
                        Token::Keyword(Keyword::Raw) => {
                            self.advance();
                            mode = DescribeMode::Raw;
                        }
                        _ => break,
                    }
                }
                self.eat_semicolon();
                Ok(Statement::ShowRelations { layer, with_examples, mode })
            }
            Token::Keyword(Keyword::Layers) => {
                self.advance();
                let range = if self.check_keyword(Keyword::Range) {
                    self.advance();
                    Some(self.parse_range()?)
                } else if matches!(self.peek(), Token::IntegerLit(_)) {
                    Some(self.parse_range()?)
                } else {
                    None
                };
                self.eat_semicolon();
                Ok(Statement::ShowLayers { range })
            }
            Token::Keyword(Keyword::Features) => {
                self.advance();
                let layer = self.expect_u32()?;

                let conditions = if self.check_keyword(Keyword::Where) {
                    self.advance();
                    self.parse_conditions()?
                } else {
                    vec![]
                };

                let limit = if self.check_keyword(Keyword::Limit) {
                    self.advance();
                    Some(self.expect_u32()?)
                } else {
                    None
                };

                self.eat_semicolon();
                Ok(Statement::ShowFeatures { layer, conditions, limit })
            }
            Token::Keyword(Keyword::Entities) => {
                self.advance();
                let layer = if self.check_keyword(Keyword::At) {
                    self.advance();
                    self.expect_keyword(Keyword::Layer)?;
                    Some(self.expect_u32()?)
                } else if matches!(self.peek(), Token::IntegerLit(_)) {
                    Some(self.expect_u32()?)
                } else {
                    None
                };
                let limit = if self.check_keyword(Keyword::Limit) {
                    self.advance();
                    Some(self.expect_u32()?)
                } else {
                    None
                };
                self.eat_semicolon();
                Ok(Statement::ShowEntities { layer, limit })
            }
            Token::Keyword(Keyword::Tokens) => {
                self.advance();
                let layer = if self.check_keyword(Keyword::At) {
                    self.advance();
                    self.expect_keyword(Keyword::Layer)?;
                    Some(self.expect_u32()?)
                } else if matches!(self.peek(), Token::IntegerLit(_)) {
                    Some(self.expect_u32()?)
                } else {
                    None
                };
                let mut conditions = vec![];
                let mut verbose = false;
                let mut group_by = None;
                let mut order_by = None;
                let mut limit = None;
                let mut export_format = None;
                loop {
                    match self.peek() {
                        Token::Keyword(Keyword::By) => {
                            self.advance();
                            if self.check_keyword(Keyword::Layer) || self.check_keyword(Keyword::Layers) {
                                self.advance();
                                group_by = Some(TokenGroupBy::Layer);
                            } else if self.check_keyword(Keyword::Band) {
                                self.advance();
                                group_by = Some(TokenGroupBy::Band);
                            } else if self.check_keyword(Keyword::Order) {
                                self.advance();
                                order_by = Some(match self.peek() {
                                    Token::Keyword(Keyword::MaxScore) => {
                                        self.advance();
                                        TokenSortBy::MaxScore
                                    }
                                    Token::Keyword(Keyword::Distinct) => {
                                        self.advance();
                                        TokenSortBy::Distinct
                                    }
                                    Token::Keyword(Keyword::Entity) => {
                                        self.advance();
                                        TokenSortBy::EntityLike
                                    }
                                    Token::Keyword(Keyword::Shape) => {
                                        self.advance();
                                        TokenSortBy::Shape
                                    }
                                    _ => return Err(ParseError(format!(
                                        "expected MAX_SCORE, DISTINCT, ENTITY, or SHAPE after ORDER BY, got {:?}",
                                        self.peek()
                                    ))),
                                });
                            } else {
                                return Err(ParseError(format!(
                                    "expected LAYER(S), BAND, or ORDER after BY, got {:?}",
                                    self.peek()
                                )));
                            }
                        }
                        Token::Keyword(Keyword::Where) => {
                            self.advance();
                            conditions = self.parse_conditions()?;
                        }
                        Token::Keyword(Keyword::Verbose) => {
                            self.advance();
                            verbose = true;
                        }
                        Token::Keyword(Keyword::Limit) => {
                            self.advance();
                            limit = Some(self.expect_u32()?);
                        }
                        Token::Keyword(Keyword::Export) => {
                            self.advance();
                            export_format = Some(match self.peek() {
                                Token::Keyword(Keyword::Csv) => {
                                    self.advance();
                                    ExportFormat::Csv
                                }
                                Token::Keyword(Keyword::Json) => {
                                    self.advance();
                                    ExportFormat::Json
                                }
                                _ => return Err(ParseError(format!(
                                    "expected CSV or JSON after EXPORT, got {:?}",
                                    self.peek()
                                ))),
                            });
                        }
                        _ => break,
                    }
                }
                self.eat_semicolon();
                Ok(Statement::ShowTokens { layer, conditions, verbose, group_by, order_by, limit, export_format })
            }
            Token::Keyword(Keyword::Models) => {
                self.advance();
                self.eat_semicolon();
                Ok(Statement::ShowModels)
            }
            Token::Keyword(Keyword::Patches) => {
                self.advance();
                self.eat_semicolon();
                Ok(Statement::ShowPatches)
            }
            _ => Err(ParseError(format!(
                "expected RELATIONS, LAYERS, FEATURES, ENTITIES, TOKENS, MODELS, or PATCHES after SHOW, got {:?}",
                self.peek()
            ))),
        }
    }

    pub(crate) fn parse_stats(&mut self) -> Result<Statement, ParseError> {
        self.expect_keyword(Keyword::Stats)?;
        let vindex = if let Token::StringLit(_) = self.peek() {
            Some(self.expect_string()?)
        } else {
            None
        };
        self.eat_semicolon();
        Ok(Statement::Stats { vindex })
    }
}
